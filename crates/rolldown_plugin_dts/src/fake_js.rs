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
//! - The original declaration source stored in a comment
//!
//! This allows Rolldown to:
//! - Track dependencies between declarations via the function references
//! - Apply tree-shaking based on which declarations are actually imported
//! - Bundle multiple `.d.ts` files into one using code splitting
//!
//! **Reverse transform** (`fake_js_to_dts`):
//! After bundling, the fake JS is converted back to valid `.d.ts` by:
//! - Extracting the original declaration source from comments
//! - Fixing import/export paths to use `.js` extensions
//! - Reassembling the declarations into a valid `.d.ts` file

use std::fmt::Write;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use oxc::allocator::Allocator;
use oxc::ast::ast::{
  BindingPattern, Declaration, ImportDeclarationSpecifier, ImportOrExportKind, Statement,
  TSModuleDeclarationName, TSTypeName, TSTypeQuery, TSTypeQueryExprName, TSTypeReference,
};
use oxc::ast_visit::{Visit, walk};
use oxc::parser::Parser;
use oxc::span::{GetSpan, SourceType};

use crate::resolver;

/// Marker used to store the original declaration source in a trailing comment.
const DTS_DECL_MARKER: &str = "__DTS_DECL__:";

/// Marker for reference directives that must be preserved at the top of the output.
const DTS_REFERENCE_MARKER: &str = "__DTS_REFERENCE__:";

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

  // Emit reference directives at the beginning
  for directive in extract_reference_directives(dts_code) {
    writeln!(output, "/* {DTS_REFERENCE_MARKER}{} */", escape_comment(directive)).ok();
  }

  for stmt in &program.body {
    let stmt_start = stmt.span().start as usize;
    let stmt_end = stmt.span().end as usize;

    match stmt {
      Statement::TSTypeAliasDeclaration(decl) => {
        let name = decl.id.name.as_str();
        let id = id_counter.fetch_add(1, Ordering::Relaxed);
        let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
        let original_source = &dts_code[stmt_start..stmt_end];
        write_fake_var(&mut output, name, id, &deps, original_source);
      }

      Statement::TSInterfaceDeclaration(decl) => {
        let name = decl.id.name.as_str();
        let id = id_counter.fetch_add(1, Ordering::Relaxed);
        let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
        let original_source = &dts_code[stmt_start..stmt_end];
        write_fake_var(&mut output, name, id, &deps, original_source);
      }

      Statement::TSEnumDeclaration(decl) => {
        let name = decl.id.name.as_str();
        let id = id_counter.fetch_add(1, Ordering::Relaxed);
        let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
        let original_source = &dts_code[stmt_start..stmt_end];
        write_fake_var(&mut output, name, id, &deps, original_source);
      }

      Statement::ClassDeclaration(decl) => {
        if let Some(ident) = &decl.id {
          let name = ident.name.as_str();
          let id = id_counter.fetch_add(1, Ordering::Relaxed);
          let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
          let original_source = &dts_code[stmt_start..stmt_end];
          write_fake_var(&mut output, name, id, &deps, original_source);
        }
      }

      Statement::VariableDeclaration(decl) => {
        for declarator in &decl.declarations {
          if let BindingPattern::BindingIdentifier(ident) = &declarator.id {
            let name = ident.name.as_str();
            let id = id_counter.fetch_add(1, Ordering::Relaxed);
            let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
            let original_source = &dts_code[stmt_start..stmt_end];
            write_fake_var(&mut output, name, id, &deps, original_source);
          }
        }
      }

      Statement::FunctionDeclaration(decl) => {
        if let Some(ident) = &decl.id {
          let name = ident.name.as_str();
          let id = id_counter.fetch_add(1, Ordering::Relaxed);
          let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
          let original_source = &dts_code[stmt_start..stmt_end];
          write_fake_var(&mut output, name, id, &deps, original_source);
        }
      }

      Statement::TSModuleDeclaration(decl) => {
        let name = match &decl.id {
          TSModuleDeclarationName::Identifier(ident) => ident.name.as_str().to_string(),
          TSModuleDeclarationName::StringLiteral(lit) => lit.value.as_str().to_string(),
        };
        let id = id_counter.fetch_add(1, Ordering::Relaxed);
        let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
        let original_source = &dts_code[stmt_start..stmt_end];
        write_fake_var(&mut output, &name, id, &deps, original_source);
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
        write_fake_var(&mut output, "__dts_default__", id, &deps, original_source);
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

      _ => {
        let original_source = &dts_code[stmt_start..stmt_end];
        if !original_source.trim().is_empty() {
          writeln!(output, "/* __DTS_PASSTHROUGH__:{} */", escape_comment(original_source)).ok();
        }
      }
    }
  }

  // Write export statement for all collected exports
  if !exports.is_empty() {
    let mut export_stmt = String::from("export { ");
    for (i, exp) in exports.iter().enumerate() {
      if i > 0 {
        export_stmt.push_str(", ");
      }
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
fn handle_exported_declaration(
  output: &mut String,
  exports: &mut Vec<ExportInfo>,
  decl: &Declaration<'_>,
  dts_code: &str,
  stmt_start: usize,
  stmt_end: usize,
  id_counter: &AtomicU32,
) {
  let deps = collect_type_deps_from_source(dts_code, stmt_start, stmt_end);
  let original_source = &dts_code[stmt_start..stmt_end];

  match decl {
    Declaration::TSTypeAliasDeclaration(d) => {
      let name = d.id.name.as_str();
      let id = id_counter.fetch_add(1, Ordering::Relaxed);
      write_fake_var(output, name, id, &deps, original_source);
      exports.push(ExportInfo::named(name, true));
    }
    Declaration::TSInterfaceDeclaration(d) => {
      let name = d.id.name.as_str();
      let id = id_counter.fetch_add(1, Ordering::Relaxed);
      write_fake_var(output, name, id, &deps, original_source);
      exports.push(ExportInfo::named(name, true));
    }
    Declaration::TSEnumDeclaration(d) => {
      let name = d.id.name.as_str();
      let id = id_counter.fetch_add(1, Ordering::Relaxed);
      write_fake_var(output, name, id, &deps, original_source);
      exports.push(ExportInfo::named(name, false));
    }
    Declaration::ClassDeclaration(d) => {
      if let Some(ident) = &d.id {
        let name = ident.name.as_str();
        let id = id_counter.fetch_add(1, Ordering::Relaxed);
        write_fake_var(output, name, id, &deps, original_source);
        exports.push(ExportInfo::named(name, false));
      }
    }
    Declaration::VariableDeclaration(d) => {
      for declarator in &d.declarations {
        if let BindingPattern::BindingIdentifier(ident) = &declarator.id {
          let name = ident.name.as_str();
          let id = id_counter.fetch_add(1, Ordering::Relaxed);
          write_fake_var(output, name, id, &deps, original_source);
          exports.push(ExportInfo::named(name, false));
        }
      }
    }
    Declaration::FunctionDeclaration(d) => {
      if let Some(ident) = &d.id {
        let name = ident.name.as_str();
        let id = id_counter.fetch_add(1, Ordering::Relaxed);
        write_fake_var(output, name, id, &deps, original_source);
        exports.push(ExportInfo::named(name, false));
      }
    }
    Declaration::TSModuleDeclaration(d) => {
      let name = match &d.id {
        TSModuleDeclarationName::Identifier(ident) => ident.name.as_str().to_string(),
        TSModuleDeclarationName::StringLiteral(lit) => lit.value.as_str().to_string(),
      };
      let id = id_counter.fetch_add(1, Ordering::Relaxed);
      write_fake_var(output, &name, id, &deps, original_source);
      exports.push(ExportInfo { local_name: name.clone(), exported_name: name, is_type: false });
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

/// Convert bundled fake JS back to valid `.d.ts` declarations.
///
/// This reads the `__DTS_DECL__`, `__DTS_PASSTHROUGH__`, and `__DTS_REFERENCE__` comments
/// to reconstruct the original declarations, then fixes import/export extensions.
pub fn fake_js_to_dts(fake_js_code: &str, _filename: &str) -> String {
  let mut output = String::new();
  let mut reference_directives: Vec<String> = Vec::new();
  let mut seen_declarations: rustc_hash::FxHashSet<String> = rustc_hash::FxHashSet::default();

  // First pass: collect reference directives
  for line in fake_js_code.lines() {
    let trimmed = line.trim();
    if let Some(ref_marker) = trimmed.find(DTS_REFERENCE_MARKER) {
      let content = &trimmed[ref_marker + DTS_REFERENCE_MARKER.len()..];
      let content = content.strip_suffix("*/").unwrap_or(content).trim();
      let unescaped = unescape_comment(content);
      if !reference_directives.contains(&unescaped) {
        reference_directives.push(unescaped);
      }
    }
  }

  // Emit reference directives at the top
  for directive in &reference_directives {
    writeln!(output, "{directive}").ok();
  }

  // Second pass: extract declarations
  for line in fake_js_code.lines() {
    let trimmed = line.trim();

    if let Some(decl_start) = trimmed.find(DTS_DECL_MARKER) {
      // Extract declaration from __DTS_DECL__ comment
      let decl_content = &trimmed[decl_start + DTS_DECL_MARKER.len()..];
      let decl = decl_content.strip_suffix("*/").unwrap_or(decl_content).trim();
      let unescaped = unescape_comment(decl);
      if seen_declarations.insert(unescaped.clone()) {
        writeln!(output, "{unescaped}").ok();
      }
    } else if let Some(pt_marker) = trimmed.find("__DTS_PASSTHROUGH__:") {
      // Extract passthrough content
      let content = &trimmed[pt_marker + "__DTS_PASSTHROUGH__:".len()..];
      let content = content.strip_suffix("*/").unwrap_or(content).trim();
      let unescaped = unescape_comment(content);
      writeln!(output, "{unescaped}").ok();
    }
    // Skip: fake JS vars, empty lines, comments, bundler artifacts, JS re-exports
  }

  // Fix import/export extensions (.d.ts -> .js)
  resolver::fix_output_extensions(&output)
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
fn write_fake_var(
  output: &mut String,
  name: &str,
  id: u32,
  deps: &[String],
  original_source: &str,
) {
  let deps_str =
    if deps.is_empty() { String::from("[]") } else { format!("[{}]", deps.join(", ")) };

  writeln!(
    output,
    "var {name} = [{id}, () => {deps_str}, [\"{name}\"]]; /* {DTS_DECL_MARKER}{} */",
    escape_comment(original_source)
  )
  .ok();
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

    assert!(result.contains("var Foo = ["));
    assert!(result.contains(DTS_DECL_MARKER));
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
    let reconstructed = fake_js_to_dts(&fake_js, "test.d.ts");

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

    // Reference directives should be preserved
    assert!(fake_js.contains(DTS_REFERENCE_MARKER));
    assert!(fake_js.contains("reference types=\"node\""));
    assert!(fake_js.contains("reference path=\"./global.d.ts\""));

    // Roundtrip should preserve reference directives at the top
    let reconstructed = fake_js_to_dts(&fake_js, "test.d.ts");
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
    let reconstructed = fake_js_to_dts(&fake_js, "test.d.ts");
    assert!(reconstructed.contains("declare global"));
    assert!(reconstructed.contains("interface Window"));
    assert!(reconstructed.contains("myProperty: string"));
  }
}
