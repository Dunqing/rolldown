use std::borrow::Cow;
use std::sync::atomic::AtomicU32;

use arcstr::ArcStr;
use dashmap::DashMap;
use oxc::{
  codegen::Codegen,
  isolated_declarations::{IsolatedDeclarations, IsolatedDeclarationsOptions},
};
use rolldown_common::{EmittedChunk, ModuleType, Output, side_effects::HookSideEffects};
use rolldown_error::{BatchedBuildDiagnostic, BuildDiagnostic, EventKind, Severity};
use rolldown_plugin::{
  HookLoadOutput, HookRenderChunkOutput, HookTransformOutput, HookUsage, Plugin, PluginHookMeta,
  PluginOrder,
};

use crate::fake_js;
use crate::options::DtsPluginOptions;
use crate::utils::{is_dts, is_dts_virtual_id, is_ts_source, make_dts_virtual_id, source_to_dts};

/// A captured TypeScript module that will produce a `.d.ts` declaration.
#[derive(Debug, Clone)]
struct DtsModule {
  /// Original source code.
  code: String,
  /// Original source file path.
  source_id: String,
  /// Whether this is an entry module.
  #[expect(dead_code)]
  is_entry: bool,
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
    Self {
      options,
      dts_map: DashMap::new(),
      declaration_id_counter: AtomicU32::new(0),
    }
  }

  /// Generate `.d.ts` code from TypeScript source using Oxc isolated declarations.
  fn generate_dts(&self, source_id: &str, code: &str) -> Result<String, BatchedBuildDiagnostic> {
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
      IsolatedDeclarationsOptions { strip_internal: self.options.strip_internal },
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

    let codegen_ret = Codegen::new().build(&ret.program);
    Ok(codegen_ret.code)
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

      // Store the source for later .d.ts generation
      self.dts_map.insert(
        virtual_id.clone(),
        DtsModule {
          code: args.code.clone(),
          source_id: id.to_string(),
          is_entry: true, // TODO: detect actual entry status
        },
      );

      // Emit a virtual chunk for the .d.ts module
      ctx.emit_chunk(EmittedChunk {
        id: virtual_id,
        name: None,
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
        side_effects: Some(HookSideEffects::False),
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
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    let specifier = args.specifier;

    // Handle virtual DTS module IDs
    if is_dts_virtual_id(specifier) {
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: ArcStr::from(specifier),
        side_effects: Some(HookSideEffects::False),
        ..Default::default()
      }));
    }

    // When importing from within a .d.ts context, resolve .ts imports to their .d.ts counterparts
    if let Some(importer) = args.importer {
      if (is_dts(importer) || is_dts_virtual_id(importer)) && is_ts_source(specifier) {
        let dts_specifier = source_to_dts(specifier);
        let virtual_id = make_dts_virtual_id(specifier);
        if self.dts_map.contains_key(&virtual_id) {
          return Ok(Some(rolldown_plugin::HookResolveIdOutput {
            id: ArcStr::from(virtual_id),
            side_effects: Some(HookSideEffects::False),
            ..Default::default()
          }));
        }
        // Try resolving as .d.ts
        return Ok(Some(rolldown_plugin::HookResolveIdOutput {
          id: ArcStr::from(dts_specifier),
          side_effects: Some(HookSideEffects::False),
          ..Default::default()
        }));
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

    // Look up the captured source module
    let dts_module = self.dts_map.get(id).map(|entry| entry.clone());

    if let Some(module) = dts_module {
      // Generate .d.ts using Oxc isolated declarations
      let dts_code = self.generate_dts(&module.source_id, &module.code)?;

      return Ok(Some(HookLoadOutput {
        code: ArcStr::from(dts_code),
        module_type: Some(ModuleType::Custom("dts".to_string())),
        side_effects: Some(HookSideEffects::False),
        ..Default::default()
      }));
    }

    Ok(None)
  }

  /// Convert bundled fake JS back to .d.ts declarations.
  async fn render_chunk(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    let filename = &args.chunk.filename;

    // Only process .d.ts output chunks
    if !is_dts(filename) {
      return Ok(None);
    }

    let reconstructed = fake_js::fake_js_to_dts(&args.code, filename);

    Ok(Some(HookRenderChunkOutput {
      code: reconstructed,
      map: None,
    }))
  }

  /// If `emit_dts_only` is set, remove non-.d.ts chunks from output.
  async fn generate_bundle(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
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
