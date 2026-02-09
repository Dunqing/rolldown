//! # Fake JS Transformation
//!
//! This module implements the core "declaration -> fake JS -> declaration" pipeline
//! that enables Rolldown to bundle `.d.ts` files using its existing JS bundling infrastructure.
//!
//! ## How it works
//!
//! **Forward transform** (`dts_to_fake_js`):
//! Each TypeScript declaration is converted to a JavaScript variable that encodes:
//! - A unique numeric declaration ID
//! - A function that returns an array of referenced type dependencies
//! - An array of child symbol names (for debugging)
//! - The original declaration source as a string literal
//! - Source file path and line number for sourcemap generation
//!
//! This allows Rolldown to:
//! - Track dependencies between declarations via the function references
//! - Apply tree-shaking based on which declarations are actually imported
//! - Bundle multiple `.d.ts` files into one using code splitting
//!
//! **Reverse transform** (`fake_js_to_dts`):
//! After bundling, the fake JS is converted back to valid `.d.ts` by:
//! - Extracting the original declaration source from string literals
//! - Extracting source file and line info for sourcemap generation
//! - Fixing import/export paths to use `.js` extensions
//! - Reassembling the declarations into a valid `.d.ts` file

use std::fmt::Write;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use oxc::allocator::Allocator;
use oxc::ast::ast::{
  BindingPattern, Declaration, ImportDeclarationSpecifier, ImportOrExportKind, Statement,
  TSModuleDeclarationName, TSModuleReference, TSTypeName, TSTypeQuery, TSTypeQueryExprName,
  TSTypeReference,
};
use oxc::ast_visit::{Visit, walk};
use oxc::parser::Parser;
use oxc::span::{GetSpan, SourceType};
use rolldown_sourcemap::{SourceMap, SourceMapBuilder};

use crate::resolver;

/// Compute 0-indexed line number from byte offset in source code.
#[expect(clippy::cast_possible_truncation)]
fn line_number_from_offset(source: &str, offset: usize) -> u32 {
  source[..offset.min(source.len())].matches('\n').count() as u32
}

/// Extract `/// <reference ... />` directives from the beginning of a `.d.ts` file.
fn extract_reference_directives(code: &str) -> Vec<&str> {
  let mut directives = Vec::new();
  for line in code.lines() {
    let trimmed = line.trim();
    if trimmed.starts_with("/// <reference") && trimmed.ends_with("/>") {
      directives.push(trimmed);
    } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
      // Stop when we hit non-comment, non-empty content
      break;
    }
  }
  directives
}

/// Convert a `.d.ts` file to fake JavaScript that preserves dependency information.
///
/// Each declaration is transformed into a variable assignment with dependency tracking,
/// allowing Rolldown's bundler to handle tree-shaking and code splitting for types.
#[expect(clippy::too_many_lines)]
pub fn dts_to_fake_js(dts_code: &str, filename: &str, id_counter: &AtomicU32) -> Result<String> {
  let allocator = Allocator::new();
  let source_type = SourceType::d_ts();
  let parser_ret = Parser::new(&allocator, dts_code, source_type).parse();

  if parser_ret.panicked {
    anyhow::bail!("Failed to parse .d.ts file: {filename}");
  }

  let program = &parser_ret.program;

  let mut output = String::new();
  let mut exports: Vec<ExportInfo> = Vec::new();
  let mut default_export: Option<String> = None;
  let mut ambient_module_counter = 0u32;

  // Emit reference directives as exported fake variables (must be exported to avoid tree-shaking)
  for (i, directive) in extract_reference_directives(dts_code).into_iter().enumerate() {
    writeln!(
      output,
      "export var __dts_ref_{i}__ = [\"__DTS_REF__\", \"{}\"];",
      escape_js_string(directive)
    )
    .ok();
  }

  for stmt in &program.body {
    let stmt_start = stmt.span().start as usize;
    let stmt_end = stmt.span().end as usize;
    let source_line = line_number_from_offset(dts_code, stmt_start);

    match stmt {
      Statement::TSTypeAliasDeclaration(decl) => {
        let name = decl.id.name.as_str();
        let id = id_counter.fetch_add(1, Ordering::Relaxed);
        let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
        let original_source = &dts_code[stmt_start..stmt_end];
        write_fake_var(&mut output, name, id, &deps, original_source, filename, source_line);
      }

      Statement::TSInterfaceDeclaration(decl) => {
        let name = decl.id.name.as_str();
        let id = id_counter.fetch_add(1, Ordering::Relaxed);
        let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
        let original_source = &dts_code[stmt_start..stmt_end];
        write_fake_var(&mut output, name, id, &deps, original_source, filename, source_line);
      }

      Statement::TSEnumDeclaration(decl) => {
        let name = decl.id.name.as_str();
        let id = id_counter.fetch_add(1, Ordering::Relaxed);
        let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
        let original_source = &dts_code[stmt_start..stmt_end];
        write_fake_var(&mut output, name, id, &deps, original_source, filename, source_line);
      }

      Statement::ClassDeclaration(decl) => {
        if let Some(ident) = &decl.id {
          let name = ident.name.as_str();
          let id = id_counter.fetch_add(1, Ordering::Relaxed);
          let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
          let original_source = &dts_code[stmt_start..stmt_end];
          write_fake_var(&mut output, name, id, &deps, original_source, filename, source_line);
        }
      }

      Statement::VariableDeclaration(decl) => {
        for declarator in &decl.declarations {
          if let BindingPattern::BindingIdentifier(ident) = &declarator.id {
            let name = ident.name.as_str();
            let id = id_counter.fetch_add(1, Ordering::Relaxed);
            let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
            let original_source = &dts_code[stmt_start..stmt_end];
            write_fake_var(&mut output, name, id, &deps, original_source, filename, source_line);
          }
        }
      }

      Statement::FunctionDeclaration(decl) => {
        if let Some(ident) = &decl.id {
          let name = ident.name.as_str();
          let id = id_counter.fetch_add(1, Ordering::Relaxed);
          let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
          let original_source = &dts_code[stmt_start..stmt_end];
          write_fake_var(&mut output, name, id, &deps, original_source, filename, source_line);
        }
      }

      Statement::TSModuleDeclaration(decl) => {
        let original_source = &dts_code[stmt_start..stmt_end];
        match &decl.id {
          TSModuleDeclarationName::Identifier(ident) => {
            // Regular namespace declaration (e.g., `namespace Foo {}`)
            let name = ident.name.as_str().to_string();
            let id = id_counter.fetch_add(1, Ordering::Relaxed);
            let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
            write_fake_var(&mut output, &name, id, &deps, original_source, filename, source_line);
          }
          TSModuleDeclarationName::StringLiteral(_) => {
            // Ambient external module declaration (e.g., `declare module 'virtual' {}`)
            // These declare types for other modules and should be passed through as-is
            // Export as a variable to prevent tree-shaking
            let idx = ambient_module_counter;
            ambient_module_counter += 1;
            writeln!(
              output,
              "export var __dts_ambient_{idx}__ = [\"__DTS_AMBIENT__\", \"{}\"];",
              escape_js_string(original_source)
            )
            .ok();
          }
        }
      }

      Statement::TSGlobalDeclaration(_) => {
        // `declare global { ... }` augments the global scope
        // Export as a variable to prevent tree-shaking
        let original_source = &dts_code[stmt_start..stmt_end];
        let idx = ambient_module_counter;
        ambient_module_counter += 1;
        writeln!(
          output,
          "export var __dts_ambient_{idx}__ = [\"__DTS_AMBIENT__\", \"{}\"];",
          escape_js_string(original_source)
        )
        .ok();
      }

      Statement::ExportNamedDeclaration(export_decl) => {
        if let Some(decl) = &export_decl.declaration {
          handle_exported_declaration(
            &mut output,
            &mut exports,
            decl,
            dts_code,
            stmt_start,
            stmt_end,
            id_counter,
            filename,
          );
        } else if let Some(source) = &export_decl.source {
          // Re-export from another module: `export { Foo } from './other'`
          let original_source = &dts_code[stmt_start..stmt_end];
          writeln!(output, "/* __DTS_PASSTHROUGH__:{} */", escape_comment(original_source)).ok();
          let mut re_export_js = String::from("export {");
          for (i, spec) in export_decl.specifiers.iter().enumerate() {
            if i > 0 {
              re_export_js.push_str(", ");
            }
            let local = spec.local.name().as_str();
            let exported = spec.exported.name().as_str();
            if local == exported {
              re_export_js.push_str(local);
            } else {
              write!(re_export_js, "{local} as {exported}").ok();
            }
          }
          write!(re_export_js, "}} from '{}';", source.value.as_str()).ok();
          writeln!(output, "{re_export_js}").ok();
        } else {
          // Local re-export: `export { Foo, Bar }`
          for spec in &export_decl.specifiers {
            let local = spec.local.name().as_str();
            let exported = spec.exported.name().as_str();
            exports.push(ExportInfo {
              local_name: local.to_string(),
              exported_name: exported.to_string(),
              is_type: matches!(spec.export_kind, ImportOrExportKind::Type),
            });
          }
        }
      }

      Statement::ExportDefaultDeclaration(_) => {
        let original_source = &dts_code[stmt_start..stmt_end];
        writeln!(output, "/* __DTS_PASSTHROUGH__:{} */", escape_comment(original_source)).ok();
        let id = id_counter.fetch_add(1, Ordering::Relaxed);
        let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
        write_fake_var(
          &mut output,
          "__dts_default__",
          id,
          &deps,
          original_source,
          filename,
          source_line,
        );
        default_export = Some("__dts_default__".to_string());
      }

      Statement::ExportAllDeclaration(export_all) => {
        let original_source = &dts_code[stmt_start..stmt_end];
        writeln!(output, "/* __DTS_PASSTHROUGH__:{} */", escape_comment(original_source)).ok();
        let source = export_all.source.value.as_str();
        if let Some(exported) = &export_all.exported {
          let name = exported.name().as_str();
          writeln!(output, "export * as {name} from '{source}';").ok();
        } else {
          writeln!(output, "export * from '{source}';").ok();
        }
      }

      Statement::ImportDeclaration(import_decl) => {
        let original_source = &dts_code[stmt_start..stmt_end];
        writeln!(output, "/* __DTS_PASSTHROUGH__:{} */", escape_comment(original_source)).ok();
        let source = import_decl.source.value.as_str();
        if let Some(specifiers) = &import_decl.specifiers {
          if specifiers.is_empty() {
            writeln!(output, "import '{source}';").ok();
          } else {
            write_import_specifiers(&mut output, specifiers, source);
          }
        }
      }

      Statement::TSImportEqualsDeclaration(decl) => {
        let original_source = &dts_code[stmt_start..stmt_end];
        writeln!(output, "/* __DTS_PASSTHROUGH__:{} */", escape_comment(original_source)).ok();
        let name = decl.id.name.as_str();
        // Convert `import Foo = require("./bar")` to `import { default as Foo } from './bar'`
        if let TSModuleReference::ExternalModuleReference(ext_ref) = &decl.module_reference {
          let source = ext_ref.expression.value.as_str();
          writeln!(output, "import {{ default as {name} }} from '{source}';").ok();
        }
      }

      Statement::TSExportAssignment(export_assign) => {
        let original_source = &dts_code[stmt_start..stmt_end];
        writeln!(output, "/* __DTS_PASSTHROUGH__:{} */", escape_comment(original_source)).ok();
        // Convert `export = Foo` to `export default Foo`
        // Extract the identifier name from the expression
        let expr_source = &dts_code[export_assign.expression.span().start as usize
          ..export_assign.expression.span().end as usize];
        writeln!(output, "export default {expr_source};").ok();
      }

      _ => {
        let original_source = &dts_code[stmt_start..stmt_end];
        if !original_source.trim().is_empty() {
          writeln!(output, "/* __DTS_PASSTHROUGH__:{} */", escape_comment(original_source)).ok();
        }
      }
    }
  }

  // Write export statement for all collected exports (deduplicate by exported name)
  if !exports.is_empty() {
    let mut seen = std::collections::HashSet::new();
    let mut export_stmt = String::from("export { ");
    let mut first = true;
    for exp in &exports {
      if !seen.insert(&exp.exported_name) {
        continue;
      }
      if !first {
        export_stmt.push_str(", ");
      }
      first = false;
      if exp.local_name == exp.exported_name {
        export_stmt.push_str(&exp.local_name);
      } else {
        write!(export_stmt, "{} as {}", exp.local_name, exp.exported_name).ok();
      }
    }
    export_stmt.push_str(" };");
    writeln!(output, "{export_stmt}").ok();
  }

  if let Some(default_name) = default_export {
    writeln!(output, "export default {default_name};").ok();
  }

  Ok(output)
}

/// Handle an exported declaration (inside `export { declaration }`).
#[expect(clippy::too_many_arguments)]
fn handle_exported_declaration(
  output: &mut String,
  exports: &mut Vec<ExportInfo>,
  decl: &Declaration<'_>,
  dts_code: &str,
  stmt_start: usize,
  stmt_end: usize,
  id_counter: &AtomicU32,
  filename: &str,
) {
  let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
  let original_source = &dts_code[stmt_start..stmt_end];
  let source_line = line_number_from_offset(dts_code, stmt_start);

  match decl {
    Declaration::TSTypeAliasDeclaration(d) => {
      let name = d.id.name.as_str();
      let id = id_counter.fetch_add(1, Ordering::Relaxed);
      write_fake_var(output, name, id, &deps, original_source, filename, source_line);
      exports.push(ExportInfo::named(name, true));
    }
    Declaration::TSInterfaceDeclaration(d) => {
      let name = d.id.name.as_str();
      let id = id_counter.fetch_add(1, Ordering::Relaxed);
      write_fake_var(output, name, id, &deps, original_source, filename, source_line);
      exports.push(ExportInfo::named(name, true));
    }
    Declaration::TSEnumDeclaration(d) => {
      let name = d.id.name.as_str();
      let id = id_counter.fetch_add(1, Ordering::Relaxed);
      write_fake_var(output, name, id, &deps, original_source, filename, source_line);
      exports.push(ExportInfo::named(name, false));
    }
    Declaration::ClassDeclaration(d) => {
      if let Some(ident) = &d.id {
        let name = ident.name.as_str();
        let id = id_counter.fetch_add(1, Ordering::Relaxed);
        write_fake_var(output, name, id, &deps, original_source, filename, source_line);
        exports.push(ExportInfo::named(name, false));
      }
    }
    Declaration::VariableDeclaration(d) => {
      for declarator in &d.declarations {
        if let BindingPattern::BindingIdentifier(ident) = &declarator.id {
          let name = ident.name.as_str();
          let id = id_counter.fetch_add(1, Ordering::Relaxed);
          write_fake_var(output, name, id, &deps, original_source, filename, source_line);
          exports.push(ExportInfo::named(name, false));
        }
      }
    }
    Declaration::FunctionDeclaration(d) => {
      if let Some(ident) = &d.id {
        let name = ident.name.as_str();
        let id = id_counter.fetch_add(1, Ordering::Relaxed);
        write_fake_var(output, name, id, &deps, original_source, filename, source_line);
        exports.push(ExportInfo::named(name, false));
      }
    }
    Declaration::TSModuleDeclaration(d) => {
      match &d.id {
        TSModuleDeclarationName::Identifier(ident) => {
          let name = ident.name.as_str().to_string();
          let id = id_counter.fetch_add(1, Ordering::Relaxed);
          write_fake_var(output, &name, id, &deps, original_source, filename, source_line);
          exports.push(ExportInfo {
            local_name: name.clone(),
            exported_name: name,
            is_type: false,
          });
        }
        TSModuleDeclarationName::StringLiteral(_) => {
          // Ambient external module declarations should be passed through
          writeln!(output, "/* __DTS_PASSTHROUGH__:{} */", escape_comment(original_source)).ok();
        }
      }
    }
    _ => {
      writeln!(output, "/* __DTS_PASSTHROUGH__:{} */", escape_comment(original_source)).ok();
    }
  }
}

/// Write JS import specifiers for Rolldown dependency tracking.
fn write_import_specifiers(
  output: &mut String,
  specifiers: &[ImportDeclarationSpecifier<'_>],
  source: &str,
) {
  // Check for namespace imports first
  for spec in specifiers {
    if let ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) = spec {
      writeln!(output, "import * as {} from '{}';", s.local.name.as_str(), source).ok();
      return;
    }
  }

  let mut import_js = String::from("import { ");
  let mut first = true;
  for spec in specifiers {
    match spec {
      ImportDeclarationSpecifier::ImportSpecifier(s) => {
        if !first {
          import_js.push_str(", ");
        }
        first = false;
        let imported = s.imported.name().as_str();
        let local = s.local.name.as_str();
        if imported == local {
          import_js.push_str(local);
        } else {
          write!(import_js, "{imported} as {local}").ok();
        }
      }
      ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
        if !first {
          import_js.push_str(", ");
        }
        first = false;
        write!(import_js, "default as {}", s.local.name.as_str()).ok();
      }
      ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {
        // Handled above
      }
    }
  }
  write!(import_js, " }} from '{source}';").ok();
  writeln!(output, "{import_js}").ok();
}

/// Information about a declaration extracted from fake JS, including source position.
#[derive(Debug, Clone)]
struct DeclSourceInfo {
  /// The original declaration source code.
  source: String,
  /// The source file this declaration came from.
  file: String,
  /// The 0-indexed line number in the source file.
  line: u32,
}

/// Convert bundled fake JS back to valid `.d.ts` declarations.
///
/// Returns (code, sourcemap) where sourcemap maps the output back to original sources.
pub fn fake_js_to_dts(fake_js_code: &str, _output_filename: &str) -> (String, Option<SourceMap>) {
  let mut output = String::new();
  let mut reference_directives: Vec<String> = Vec::new();
  let mut seen_declarations: rustc_hash::FxHashSet<String> = rustc_hash::FxHashSet::default();

  // Sourcemap builder for tracking source positions
  let mut sourcemap_builder = SourceMapBuilder::default();
  let mut current_line: u32 = 0;

  // Track source IDs for each unique source file
  let mut source_ids: rustc_hash::FxHashMap<String, u32> = rustc_hash::FxHashMap::default();

  // First pass: collect reference directives from fake variables
  // Format: var __dts_ref__ = ["__DTS_REF__", "/// <reference...>"];
  for directive in extract_reference_directives_from_fake_js(fake_js_code) {
    if !reference_directives.contains(&directive) {
      reference_directives.push(directive);
    }
  }

  // Collect ambient module declarations
  // Format: var __dts_ambient__ = ["__DTS_AMBIENT__", "declare module 'x' {...}"];
  let ambient_modules = extract_ambient_modules_from_fake_js(fake_js_code);

  // Emit reference directives at the top (no sourcemap for these)
  for directive in &reference_directives {
    writeln!(output, "{directive}").ok();
    current_line += 1;
  }

  // Emit ambient module declarations (no sourcemap for these)
  for module_decl in &ambient_modules {
    writeln!(output, "{module_decl}").ok();
    #[expect(clippy::cast_possible_truncation)]
    let line_count = module_decl.matches('\n').count() as u32 + 1;
    current_line += line_count;
  }

  // Second pass: extract passthrough comments (line by line)
  for line in fake_js_code.lines() {
    let trimmed = line.trim();
    if let Some(pt_marker) = trimmed.find("__DTS_PASSTHROUGH__:") {
      let content = &trimmed[pt_marker + "__DTS_PASSTHROUGH__:".len()..];
      let content = content.strip_suffix("*/").unwrap_or(content).trim();
      let unescaped = unescape_comment(content);
      #[expect(clippy::cast_possible_truncation)]
      let line_count = unescaped.matches('\n').count() as u32 + 1;
      writeln!(output, "{unescaped}").ok();
      current_line += line_count;
    }
  }

  // Third pass: extract declaration sources from fake JS arrays with source info
  // Format: [id, () => deps, ["name"], "source", "file", line]
  for decl_info in extract_all_decl_sources_with_info(fake_js_code) {
    // Skip reference directives (already handled above)
    if decl_info.source.starts_with("/// <reference") {
      continue;
    }
    if seen_declarations.insert(decl_info.source.clone()) {
      // Get or create source ID for this file (0 if no file info)
      let source_id = if decl_info.file.is_empty() {
        0
      } else {
        *source_ids
          .entry(decl_info.file.clone())
          .or_insert_with(|| sourcemap_builder.set_source_and_content(&decl_info.file, ""))
      };

      // Add sourcemap token for each line of the declaration
      if !decl_info.file.is_empty() {
        sourcemap_builder.add_token(current_line, 0, decl_info.line, 0, Some(source_id), None);
      }

      writeln!(output, "{}", decl_info.source).ok();
      #[expect(clippy::cast_possible_truncation)]
      let line_count = decl_info.source.matches('\n').count() as u32 + 1;
      current_line += line_count;
    }
  }

  // Fix import/export extensions (.d.ts -> .js)
  let final_code = resolver::fix_output_extensions(&output);

  // Build the sourcemap (only if we have actual source mappings)
  let sourcemap =
    if source_ids.is_empty() { None } else { Some(sourcemap_builder.into_sourcemap()) };

  (final_code, sourcemap)
}

/// Information about an exported symbol.
#[derive(Debug)]
struct ExportInfo {
  local_name: String,
  exported_name: String,
  #[expect(dead_code)]
  is_type: bool,
}

impl ExportInfo {
  fn named(name: &str, is_type: bool) -> Self {
    Self { local_name: name.to_string(), exported_name: name.to_string(), is_type }
  }
}

/// Write a fake JS variable declaration that encodes a TypeScript declaration.
/// Format: `var name = [id, () => [deps], ["name"], "escaped_source", "source_file", line];`
/// The source info (file + line) enables sourcemap generation during reconstruction.
fn write_fake_var(
  output: &mut String,
  name: &str,
  id: u32,
  deps: &[String],
  original_source: &str,
  source_file: &str,
  source_line: u32,
) {
  let deps_str =
    if deps.is_empty() { String::from("[]") } else { format!("[{}]", deps.join(", ")) };

  // Escape the source for embedding as a JS string literal
  let escaped_source = escape_js_string(original_source);
  let escaped_file = escape_js_string(source_file);

  writeln!(
    output,
    "var {name} = [{id}, () => {deps_str}, [\"{name}\"], \"{escaped_source}\", \"{escaped_file}\", {source_line}];",
  )
  .ok();
}

/// Escape a string for use as a JavaScript string literal.
fn escape_js_string(s: &str) -> String {
  s.replace('\\', "\\\\")
    .replace('"', "\\\"")
    .replace('\n', "\\n")
    .replace('\r', "\\r")
    .replace('\t', "\\t")
}

/// Unescape a JavaScript string literal back to the original string.
fn unescape_js_string(s: &str) -> String {
  let mut result = String::new();
  let mut chars = s.chars();
  while let Some(c) = chars.next() {
    if c == '\\' {
      match chars.next() {
        Some('n') => result.push('\n'),
        Some('r') => result.push('\r'),
        Some('t') => result.push('\t'),
        Some('"') => result.push('"'),
        Some('\\') | None => result.push('\\'),
        Some(other) => {
          result.push('\\');
          result.push(other);
        }
      }
    } else {
      result.push(c);
    }
  }
  result
}

/// Extract reference directives from fake variables.
/// Format: `var __dts_ref__ = ["__DTS_REF__", "/// <reference...>"];`
fn extract_reference_directives_from_fake_js(code: &str) -> Vec<String> {
  let mut directives = Vec::new();
  let marker = "[\"__DTS_REF__\",";

  let mut search_start = 0;
  while let Some(pos) = code[search_start..].find(marker) {
    let abs_pos = search_start + pos;
    let after_marker = &code[abs_pos + marker.len()..];
    let trimmed = after_marker.trim_start();

    if let Some(quote_content) = trimmed.strip_prefix('"') {
      if let Some(source) = extract_quoted_string(quote_content) {
        directives.push(source);
      }
    }

    search_start = abs_pos + marker.len();
  }

  directives
}

/// Extract ambient module declarations from fake variables.
/// Format: `var __dts_ambient__ = ["__DTS_AMBIENT__", "declare module 'x' {...}"];`
fn extract_ambient_modules_from_fake_js(code: &str) -> Vec<String> {
  let mut modules = Vec::new();
  let marker = "[\"__DTS_AMBIENT__\",";

  let mut search_start = 0;
  while let Some(pos) = code[search_start..].find(marker) {
    let abs_pos = search_start + pos;
    let after_marker = &code[abs_pos + marker.len()..];
    let trimmed = after_marker.trim_start();

    if let Some(quote_content) = trimmed.strip_prefix('"') {
      if let Some(source) = extract_quoted_string(quote_content) {
        modules.push(source);
      }
    }

    search_start = abs_pos + marker.len();
  }

  modules
}

/// Extract all declaration sources with source position info from fake JS code.
/// Format: `var NAME = [id, () => deps, ["name"], "source", "file", line];`
/// Falls back to default values for legacy format without source info.
fn extract_all_decl_sources_with_info(code: &str) -> Vec<DeclSourceInfo> {
  let mut results = Vec::new();

  // Find all patterns like: ], followed by whitespace/newline, then "source"
  // Pattern: ],\n\t"escaped_source", "file", line\n];
  let mut search_start = 0;
  while let Some(pos) = code[search_start..].find("],") {
    let abs_pos = search_start + pos;

    // Skip the '],' and any whitespace
    let after_bracket = &code[abs_pos + 2..];
    let trimmed = after_bracket.trim_start();

    // Check if next non-whitespace is a quote (source string)
    if let Some(quote_content) = trimmed.strip_prefix('"') {
      if let Some((source, rest)) = extract_quoted_string_with_rest(quote_content) {
        // Try to extract file and line info
        // Format after source: , "file", line];
        let (file, line) = extract_source_info(rest);

        results.push(DeclSourceInfo { source, file, line });
      }
    }

    search_start = abs_pos + 1;
  }

  results
}

/// Extract source file and line from the rest of a fake JS array.
/// Input: `, "file", 42];` or similar
fn extract_source_info(rest: &str) -> (String, u32) {
  let trimmed = rest.trim_start();

  // Expect: , "file", line
  let Some(after_comma) = trimmed.strip_prefix(',') else {
    return (String::new(), 0);
  };

  let trimmed = after_comma.trim_start();
  let Some(quote_content) = trimmed.strip_prefix('"') else {
    return (String::new(), 0);
  };

  let Some((file, rest)) = extract_quoted_string_with_rest(quote_content) else {
    return (String::new(), 0);
  };

  // Extract line number
  let trimmed = rest.trim_start();
  let Some(after_comma) = trimmed.strip_prefix(',') else {
    return (file, 0);
  };

  let trimmed = after_comma.trim_start();
  // Parse digits until non-digit
  let line_str: String = trimmed.chars().take_while(char::is_ascii_digit).collect();
  let line = line_str.parse().unwrap_or(0);

  (file, line)
}

/// Extract a quoted string, handling escape sequences.
/// Input should start right after the opening quote.
fn extract_quoted_string(content: &str) -> Option<String> {
  extract_quoted_string_with_rest(content).map(|(s, _)| s)
}

/// Extract a quoted string and return the remaining text after the closing quote.
/// Input should start right after the opening quote.
fn extract_quoted_string_with_rest(content: &str) -> Option<(String, &str)> {
  let mut in_escape = false;
  let mut end_pos = None;

  for (i, c) in content.char_indices() {
    if in_escape {
      in_escape = false;
    } else if c == '\\' {
      in_escape = true;
    } else if c == '"' {
      end_pos = Some(i);
      break;
    }
  }

  let end = end_pos?;
  let escaped_source = &content[..end];
  let rest = &content[end + 1..]; // Skip the closing quote
  Some((unescape_js_string(escaped_source), rest))
}

/// Collect type dependency names referenced in a span of `.d.ts` source.
fn collect_type_deps_from_source(source: &str, start: usize, end: usize) -> Vec<String> {
  let span_source = &source[start..end];
  let allocator = Allocator::new();
  let source_type = SourceType::d_ts();

  let parser_ret = Parser::new(&allocator, span_source, source_type).parse();
  if parser_ret.panicked {
    return vec![];
  }

  let mut collector = TypeDepCollector { deps: Vec::new() };
  collector.visit_program(&parser_ret.program);

  collector.deps.sort();
  collector.deps.dedup();
  collector.deps
}

/// AST visitor that collects type reference identifiers.
struct TypeDepCollector {
  deps: Vec<String>,
}

impl<'a> Visit<'a> for TypeDepCollector {
  fn visit_ts_type_reference(&mut self, it: &TSTypeReference<'a>) {
    if let TSTypeName::IdentifierReference(ident) = &it.type_name {
      let name = ident.name.as_str();
      if !is_builtin_type(name) {
        self.deps.push(name.to_string());
      }
    }
    walk::walk_ts_type_reference(self, it);
  }

  fn visit_ts_type_query(&mut self, it: &TSTypeQuery<'a>) {
    if let TSTypeQueryExprName::IdentifierReference(ident) = &it.expr_name {
      let name = ident.name.as_str();
      if !is_builtin_type(name) {
        self.deps.push(name.to_string());
      }
    }
    walk::walk_ts_type_query(self, it);
  }
}

/// Check if a type name is a built-in TypeScript type that doesn't need tracking.
fn is_builtin_type(name: &str) -> bool {
  matches!(
    name,
    "string"
      | "number"
      | "boolean"
      | "symbol"
      | "bigint"
      | "undefined"
      | "null"
      | "void"
      | "never"
      | "unknown"
      | "any"
      | "object"
      | "Array"
      | "Promise"
      | "Map"
      | "Set"
      | "WeakMap"
      | "WeakSet"
      | "Record"
      | "Partial"
      | "Required"
      | "Readonly"
      | "Pick"
      | "Omit"
      | "Exclude"
      | "Extract"
      | "NonNullable"
      | "ReturnType"
      | "InstanceType"
      | "Parameters"
      | "ConstructorParameters"
      | "ThisParameterType"
      | "OmitThisParameter"
      | "ThisType"
      | "Uppercase"
      | "Lowercase"
      | "Capitalize"
      | "Uncapitalize"
      | "Awaited"
      | "NoInfer"
  )
}

/// Escape `*/` in a string so it can be safely embedded in a block comment.
fn escape_comment(s: &str) -> String {
  s.replace("*/", "*\\/").replace('\n', "\\n").replace('\r', "")
}

/// Unescape a comment-safe string back to the original.
fn unescape_comment(s: &str) -> String {
  s.replace("*\\/", "*/").replace("\\n", "\n")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_simple_dts_to_fake_js() {
    let dts = "export interface Foo {\n  x: number;\n}\n";
    let counter = AtomicU32::new(0);
    let result = dts_to_fake_js(dts, "test.d.ts", &counter).unwrap();

    // Check that fake JS variable is created
    assert!(result.contains("var Foo = ["), "result: {result}");
    // Check that the original source is embedded in the array (escaped)
    assert!(result.contains("export interface Foo"), "result: {result}");
    assert!(result.contains("export {"));
  }

  #[test]
  fn test_type_alias_with_deps() {
    let dts = "type Bar = Foo & { y: string };\n";
    let counter = AtomicU32::new(0);
    let result = dts_to_fake_js(dts, "test.d.ts", &counter).unwrap();

    assert!(result.contains("var Bar = ["));
    assert!(result.contains("Foo"));
  }

  #[test]
  fn test_roundtrip() {
    let dts = "export declare const x: number;\n";
    let counter = AtomicU32::new(0);
    let fake_js = dts_to_fake_js(dts, "test.d.ts", &counter).unwrap();
    let (reconstructed, _map) = fake_js_to_dts(&fake_js, "test.d.ts");

    assert!(reconstructed.contains("export declare const x: number;"));
  }

  #[test]
  fn test_escape_unescape() {
    let original = "interface Foo { /* comment */ x: number }";
    let escaped = escape_comment(original);
    let unescaped = unescape_comment(&escaped);
    assert_eq!(original, unescaped);
  }

  #[test]
  fn test_builtin_types_not_tracked() {
    assert!(is_builtin_type("string"));
    assert!(is_builtin_type("Array"));
    assert!(is_builtin_type("Promise"));
    assert!(!is_builtin_type("MyCustomType"));
  }

  #[test]
  fn test_reference_directives() {
    let dts = r#"/// <reference types="node" />
/// <reference path="./global.d.ts" />

export interface Foo {
  x: number;
}
"#;
    let counter = AtomicU32::new(0);
    let fake_js = dts_to_fake_js(dts, "test.d.ts", &counter).unwrap();

    // Reference directives should be preserved as exported fake variables
    assert!(fake_js.contains("export var __dts_ref_"));
    assert!(fake_js.contains("__DTS_REF__"));
    assert!(fake_js.contains("reference types=\\\"node\\\""));
    assert!(fake_js.contains("reference path=\\\"./global.d.ts\\\""));

    // Roundtrip should preserve reference directives at the top
    let (reconstructed, _map) = fake_js_to_dts(&fake_js, "test.d.ts");
    assert!(reconstructed.contains("/// <reference types=\"node\""));
    assert!(reconstructed.contains("/// <reference path=\"./global.d.ts\""));
  }

  #[test]
  fn test_declare_global() {
    let dts = "declare global {
  interface Window {
    myProperty: string;
  }
}

export interface Foo {
  x: number;
}
";
    let counter = AtomicU32::new(0);
    let fake_js = dts_to_fake_js(dts, "test.d.ts", &counter).unwrap();

    // declare global should be preserved
    assert!(fake_js.contains("declare global"));

    // Roundtrip should preserve declare global
    let (reconstructed, _map) = fake_js_to_dts(&fake_js, "test.d.ts");
    assert!(reconstructed.contains("declare global"));
    assert!(reconstructed.contains("interface Window"));
    assert!(reconstructed.contains("myProperty: string"));
  }
}
