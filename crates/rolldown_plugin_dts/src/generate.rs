use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::AtomicU32;

use arcstr::ArcStr;
use dashmap::DashMap;
use oxc::{
  codegen::{Codegen, CodegenOptions},
  isolated_declarations::{IsolatedDeclarations, IsolatedDeclarationsOptions},
};
use rolldown_common::{EmittedChunk, ModuleType, Output, side_effects::HookSideEffects};
use rolldown_error::{BatchedBuildDiagnostic, BuildDiagnostic, EventKind, Severity};
use rolldown_plugin::{
  HookLoadOutput, HookRenderChunkArgs, HookRenderChunkOutput, HookTransformOutput, HookUsage,
  Plugin, PluginHookMeta, PluginOrder,
};

use crate::fake_js;
use crate::options::DtsPluginOptions;
use crate::utils::{
  dts_to_source, is_dts, is_dts_virtual_id, is_ts_source, make_dts_virtual_id, source_to_dts,
  virtual_id_to_dts_path,
};

/// Check if the code contains fake DTS variable declarations.
/// Looks for patterns indicating our fake JS format, which may be reformatted across lines.
fn is_fake_dts_var(code: &str) -> bool {
  // Check for the array pattern with embedded source string
  // Rolldown may reformat this across lines, so we check the whole code
  // Look for: ["name"],\n\t"source" patterns (the last two elements of our array)
  // Also check for reference directives and ambient modules
  code.contains("\"],\n\t\"")
    || code.contains("\"],\n  \"")
    || code.contains("\"], \"")
    || code.contains("__DTS_REF__")
    || code.contains("__DTS_AMBIENT__")
}

/// Convert a JS filename to its DTS equivalent.
/// e.g., "main.js" -> "main.d.ts", "main.d.js" -> "main.d.ts"
fn convert_js_to_dts_filename(filename: &str) -> String {
  // Check for .d.js/.d.mjs/.d.cjs first (more specific patterns)
  if let Some(stem) = filename.strip_suffix(".d.js") {
    format!("{stem}.d.ts")
  } else if let Some(stem) = filename.strip_suffix(".d.mjs") {
    format!("{stem}.d.mts")
  } else if let Some(stem) = filename.strip_suffix(".d.cjs") {
    format!("{stem}.d.cts")
  } else if let Some(stem) = filename.strip_suffix(".js") {
    format!("{stem}.d.ts")
  } else if let Some(stem) = filename.strip_suffix(".mjs") {
    format!("{stem}.d.mts")
  } else if let Some(stem) = filename.strip_suffix(".cjs") {
    format!("{stem}.d.cts")
  } else {
    // Fallback: just add .d.ts
    format!("{filename}.d.ts")
  }
}

/// A captured TypeScript module that will produce a `.d.ts` declaration.
#[derive(Debug, Clone)]
struct DtsModule {
  /// Original source code.
  code: String,
  /// Original source file path.
  source_id: String,
}

/// The main DTS plugin that generates and bundles `.d.ts` declaration files.
///
/// This plugin works by:
/// 1. Capturing `.ts` source files during `transform` (pre-order)
/// 2. Emitting virtual `.d.ts` entry chunks for each captured source
/// 3. Generating `.d.ts` code via Oxc's isolated declarations in `load`
/// 4. Converting `.d.ts` to "fake JS" in `transform` so Rolldown can bundle declarations
/// 5. Reconstructing `.d.ts` from bundled fake JS in `render_chunk`
/// 6. Optionally removing JS output chunks if `emit_dts_only` is set
#[derive(Debug)]
pub struct DtsPlugin {
  options: DtsPluginOptions,
  /// Map of virtual DTS module ID -> captured source module info.
  dts_map: DashMap<String, DtsModule>,
  /// Counter for generating unique declaration IDs in fake JS.
  declaration_id_counter: AtomicU32,
}

impl DtsPlugin {
  pub fn new(options: DtsPluginOptions) -> Self {
    Self { options, dts_map: DashMap::new(), declaration_id_counter: AtomicU32::new(0) }
  }

  /// Returns the `HookSideEffects` value based on the `side_effects` option.
  /// When `side_effects` is false (default), returns `Some(False)` for tree-shaking.
  /// When `side_effects` is true, returns `None` to let Rolldown use its default behavior.
  fn hook_side_effects(&self) -> Option<HookSideEffects> {
    if self.options.side_effects { None } else { Some(HookSideEffects::False) }
  }

  /// Generate `.d.ts` code from TypeScript source using Oxc isolated declarations.
  /// Always produces sourcemaps so rolldown can compose them in the sourcemap chain.
  fn generate_dts(
    &self,
    source_id: &str,
    code: &str,
  ) -> Result<(String, Option<rolldown_sourcemap::SourceMap>), BatchedBuildDiagnostic> {
    let allocator = oxc::allocator::Allocator::new();
    let source_type = oxc::span::SourceType::from_path(source_id).unwrap_or_default();
    let parser_ret = oxc::parser::Parser::new(&allocator, code, source_type).parse();

    if parser_ret.panicked || !parser_ret.errors.is_empty() {
      return Err(BatchedBuildDiagnostic::new(BuildDiagnostic::from_oxc_diagnostics(
        parser_ret.errors,
        &ArcStr::from(code),
        source_id,
        Severity::Error,
        EventKind::ParseError,
      )));
    }

    let ret = IsolatedDeclarations::new(
      &allocator,
      IsolatedDeclarationsOptions { strip_internal: self.options.compiler_options.strip_internal },
    )
    .build(&parser_ret.program);

    if !ret.errors.is_empty() {
      return Err(BatchedBuildDiagnostic::new(BuildDiagnostic::from_oxc_diagnostics(
        ret.errors,
        &ArcStr::from(ret.program.source_text),
        source_id,
        Severity::Error,
        EventKind::ParseError,
      )));
    }

    let codegen_ret = Codegen::new()
      .with_options(CodegenOptions {
        source_map_path: Some(PathBuf::from(source_id)),
        ..CodegenOptions::default()
      })
      .build(&ret.program);

    Ok((codegen_ret.code, codegen_ret.map))
  }
}

impl Plugin for DtsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:dts")
  }

  // --- Build hooks ---

  /// Capture `.ts` source files and emit virtual `.d.ts` entry chunks.
  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let id = args.id;

    // Phase 1: Capture TypeScript source files (runs on .ts/.tsx files)
    if is_ts_source(id) {
      let virtual_id = make_dts_virtual_id(id);

      // Store the source with BOTH the virtual ID and source path as keys
      // This ensures we can find it regardless of how it's looked up
      let module = DtsModule { code: args.code.clone(), source_id: id.to_string() };
      self.dts_map.insert(virtual_id.clone(), module.clone());
      self.dts_map.insert(id.to_string(), module);

      // Extract base name from the source file (e.g., "main" from "main.ts")
      let entry_name =
        std::path::Path::new(id).file_stem().and_then(|s| s.to_str()).map(ArcStr::from);

      // Emit a virtual chunk for the .d.ts module
      ctx
        .emit_chunk(EmittedChunk {
          id: virtual_id,
          name: entry_name,
          file_name: None,
          importer: Some(id.to_string()),
          preserve_entry_signatures: None,
        })
        .await?;

      // Don't modify the original source
      return Ok(None);
    }

    // Phase 2: Convert .d.ts code to fake JS for bundling
    if is_dts(id) || is_dts_virtual_id(id) {
      let dts_code = args.code;
      let fake_js_code = fake_js::dts_to_fake_js(dts_code, id, &self.declaration_id_counter)?;

      return Ok(Some(HookTransformOutput {
        code: Some(fake_js_code),
        module_type: Some(ModuleType::Js),
        side_effects: self.hook_side_effects(),
        ..Default::default()
      }));
    }

    Ok(None)
  }

  fn transform_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
  }

  /// Resolve virtual DTS module IDs.
  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    let specifier = args.specifier;

    // Handle virtual DTS module IDs
    if is_dts_virtual_id(specifier) {
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: ArcStr::from(specifier),
        side_effects: self.hook_side_effects(),
        ..Default::default()
      }));
    }

    // When importing from within a .d.ts context, resolve imports appropriately
    if let Some(importer) = args.importer {
      let in_dts_context = is_dts(importer) || is_dts_virtual_id(importer);

      if in_dts_context {
        // Get the real importer path for resolution
        // - For virtual IDs (\0dts:/path/to/file.d.ts), use the source file (/path/to/file.ts)
        // - For real .d.ts files, use the .d.ts file directly
        let real_importer = if is_dts_virtual_id(importer) {
          let dts_path = virtual_id_to_dts_path(importer);
          dts_to_source(dts_path)
        } else {
          importer.to_string()
        };

        // If specifier is a .ts file, resolve to its .d.ts counterpart
        if is_ts_source(specifier) {
          let virtual_id = make_dts_virtual_id(specifier);
          if self.dts_map.contains_key(&virtual_id) {
            return Ok(Some(rolldown_plugin::HookResolveIdOutput {
              id: ArcStr::from(virtual_id),
              side_effects: self.hook_side_effects(),
              ..Default::default()
            }));
          }
          // Try resolving as .d.ts
          let dts_specifier = source_to_dts(specifier);
          return Ok(Some(rolldown_plugin::HookResolveIdOutput {
            id: ArcStr::from(dts_specifier),
            side_effects: self.hook_side_effects(),
            ..Default::default()
          }));
        }

        // For relative imports with .js/.mjs/.cjs extensions, resolve to corresponding .d.ts
        if specifier.starts_with('.') {
          let dts_specifier = specifier
            .strip_suffix(".js")
            .map(|stem| format!("{stem}.d.ts"))
            .or_else(|| specifier.strip_suffix(".mjs").map(|stem| format!("{stem}.d.mts")))
            .or_else(|| specifier.strip_suffix(".cjs").map(|stem| format!("{stem}.d.cts")));
          if let Some(dts_specifier) = dts_specifier {
            let importer_dir =
              std::path::Path::new(&real_importer).parent().unwrap_or(std::path::Path::new("."));
            let dts_path = importer_dir.join(&dts_specifier);
            if dts_path.exists() {
              return Ok(Some(rolldown_plugin::HookResolveIdOutput {
                id: ArcStr::from(dts_path.to_string_lossy().to_string()),
                side_effects: self.hook_side_effects(),
                ..Default::default()
              }));
            }
          }
        }

        // For relative imports without extensions, try to resolve as .d.ts
        // Check if the specifier has no file extension (last path segment has no dot)
        // Note: "." and ".." are directory imports and count as extensionless
        let last_segment = specifier.rsplit('/').next().unwrap_or("");
        let is_extensionless = specifier.starts_with('.')
          && (last_segment == "." || last_segment == ".." || !last_segment.contains('.'));
        if is_extensionless {
          // First, try the normal resolver with the source importer path
          if let Ok(Ok(resolved)) = ctx.resolve(specifier, Some(&real_importer), None).await {
            // If resolved to a .ts file, convert to virtual DTS ID
            let resolved_id = resolved.id.into_inner();
            if is_ts_source(&resolved_id) {
              let virtual_id = make_dts_virtual_id(&resolved_id);
              return Ok(Some(rolldown_plugin::HookResolveIdOutput {
                id: ArcStr::from(virtual_id),
                side_effects: self.hook_side_effects(),
                ..Default::default()
              }));
            }
            return Ok(Some(rolldown_plugin::HookResolveIdOutput {
              id: resolved_id,
              side_effects: self.hook_side_effects(),
              ..Default::default()
            }));
          }

          // If normal resolution fails, compute absolute path
          let importer_dir =
            std::path::Path::new(&real_importer).parent().unwrap_or(std::path::Path::new("."));

          // For directory imports ("." or ".."), resolve to index file
          let is_dir_import = specifier == "." || specifier == ".." || specifier.ends_with("/.");
          if is_dir_import {
            let dir = importer_dir.join(specifier);
            // Try index.d.ts first (for real .d.ts importers)
            let index_dts = dir.join("index.d.ts");
            if index_dts.exists() {
              return Ok(Some(rolldown_plugin::HookResolveIdOutput {
                id: ArcStr::from(index_dts.to_string_lossy().to_string()),
                side_effects: self.hook_side_effects(),
                ..Default::default()
              }));
            }
          }

          // Strip leading "./" for file-based resolution
          let base_name = specifier.strip_prefix("./").unwrap_or(specifier);

          // For virtual DTS importers, resolve to virtual DTS ID
          // For real .d.ts importers, resolve to .d.ts file
          if is_dts_virtual_id(importer) {
            let ts_path = importer_dir.join(format!("{base_name}.ts"));
            let virtual_id = make_dts_virtual_id(&ts_path.to_string_lossy());
            return Ok(Some(rolldown_plugin::HookResolveIdOutput {
              id: ArcStr::from(virtual_id),
              side_effects: self.hook_side_effects(),
              ..Default::default()
            }));
          }
          // Real .d.ts file, resolve to .d.ts
          let dts_path = importer_dir.join(format!("{base_name}.d.ts"));
          return Ok(Some(rolldown_plugin::HookResolveIdOutput {
            id: ArcStr::from(dts_path.to_string_lossy().to_string()),
            side_effects: self.hook_side_effects(),
            ..Default::default()
          }));
        }
      }
    }

    Ok(None)
  }

  fn resolve_id_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
  }

  /// Load virtual DTS modules by generating .d.ts from captured source.
  async fn load(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    let id = args.id;

    if !is_dts_virtual_id(id) {
      return Ok(None);
    }

    // Convert virtual ID to source path
    let dts_path = virtual_id_to_dts_path(id);
    let source_path = dts_to_source(dts_path);

    // Try to find captured source in dts_map first
    let module =
      self.dts_map.get(id).or_else(|| self.dts_map.get(&source_path)).as_deref().cloned();
    if let Some(module) = module {
      let (dts_code, map) = self.generate_dts(&module.source_id, &module.code)?;
      return Ok(Some(HookLoadOutput {
        code: ArcStr::from(dts_code),
        map,
        module_type: Some(ModuleType::Custom("dts".to_string())),
        side_effects: self.hook_side_effects(),
      }));
    }

    // If not in dts_map, try reading the source file from disk
    // This handles the case where dependencies haven't been transformed yet
    if let Ok(code) = std::fs::read_to_string(&source_path) {
      let (dts_code, map) = self.generate_dts(&source_path, &code)?;
      return Ok(Some(HookLoadOutput {
        code: ArcStr::from(dts_code),
        map,
        module_type: Some(ModuleType::Custom("dts".to_string())),
        side_effects: self.hook_side_effects(),
      }));
    }

    // Try loading a real .d.ts file if it exists on disk
    // This handles imports of existing .d.ts files (not generated from .ts sources)
    if let Ok(code) = std::fs::read_to_string(dts_path) {
      return Ok(Some(HookLoadOutput {
        code: ArcStr::from(code),
        map: None,
        module_type: Some(ModuleType::Custom("dts".to_string())),
        side_effects: self.hook_side_effects(),
      }));
    }

    Ok(None)
  }

  /// Convert bundled fake JS back to .d.ts declarations.
  /// Rolldown composes the sourcemap chain automatically.
  async fn render_chunk(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    let is_dts_chunk = args.code.contains("__DTS_PASSTHROUGH__:") || is_fake_dts_var(&args.code);

    if !is_dts_chunk {
      return Ok(None);
    }

    let dts_code = fake_js::fake_js_to_dts(&args.code, self.options.cjs_default);

    Ok(Some(HookRenderChunkOutput { code: dts_code, map: None }))
  }

  /// Rename output files to .d.ts and optionally remove JS chunks.
  async fn generate_bundle(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    // Rename DTS chunks from .js to .d.ts
    for output in args.bundle.iter_mut() {
      if let Output::Chunk(chunk_arc) = output {
        // Detect DTS chunks by checking if code looks like declarations (not fake JS)
        let code = &chunk_arc.code;
        let looks_like_dts = code.contains("declare ")
          || code.contains("export interface")
          || code.contains("export type")
          || code.contains("/// <reference");

        if looks_like_dts && !is_dts(&chunk_arc.filename) {
          let chunk_ref: &rolldown_common::OutputChunk = chunk_arc;
          let mut chunk = chunk_ref.clone();
          let new_filename = convert_js_to_dts_filename(&chunk.filename);

          // Update sourcemap filename if a sourcemap exists
          if chunk.map.is_some() {
            chunk.sourcemap_filename = Some(format!("{new_filename}.map"));
          }

          chunk.filename = ArcStr::from(new_filename);
          *output = Output::Chunk(std::sync::Arc::new(chunk));
        }
      }
    }

    // If emit_dts_only is set, remove non-.d.ts chunks from output
    if self.options.emit_dts_only {
      args.bundle.retain(|output| match output {
        Output::Chunk(chunk) => is_dts(&chunk.filename),
        Output::Asset(_) => true,
      });
    }
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
      | HookUsage::ResolveId
      | HookUsage::Load
      | HookUsage::RenderChunk
      | HookUsage::GenerateBundle
  }
}
