use crate::utils::{is_dts, source_to_dts};

/// Resolve an import specifier within a `.d.ts` context.
///
/// When bundling `.d.ts` files, import paths need special handling:
/// - Source file imports (`.ts`) are redirected to their `.d.ts` counterparts
/// - Package imports are resolved to their type definitions
/// - Relative `.d.ts` imports are passed through
#[expect(dead_code, reason = "will be used when resolver is fully integrated")]
pub fn resolve_dts_import(specifier: &str, _importer: &str) -> Option<String> {
  // If already a .d.ts import, pass through
  if is_dts(specifier) {
    return Some(specifier.to_string());
  }

  // If it's a relative TypeScript import, redirect to .d.ts
  if specifier.starts_with('.') || specifier.starts_with('/') {
    if specifier.ends_with(".ts") || specifier.ends_with(".tsx") {
      return Some(source_to_dts(specifier));
    }
    // For extensionless relative imports, try .d.ts
    if !specifier.contains('.') || specifier.ends_with('/') {
      return Some(format!("{specifier}.d.ts"));
    }
  }

  // Package imports - these would need node_modules resolution
  // For now, mark as external
  None
}

/// Convert declaration import paths in output to use `.js` extensions.
/// e.g., `import { Foo } from './bar.d.ts'` -> `import { Foo } from './bar.js'`
///
/// Note: This does NOT modify reference directives (`/// <reference ...>`)
/// since those should continue to reference `.d.ts` files.
pub fn fix_output_extensions(code: &str) -> String {
  let mut result = String::new();
  for line in code.lines() {
    let trimmed = line.trim();
    // Don't modify reference directives - they should keep .d.ts extensions
    if trimmed.starts_with("/// <reference") {
      result.push_str(line);
    } else {
      // Fix import/export extensions
      let fixed = line
        .replace(".d.ts'", ".js'")
        .replace(".d.ts\"", ".js\"")
        .replace(".d.mts'", ".mjs'")
        .replace(".d.mts\"", ".mjs\"")
        .replace(".d.cts'", ".cjs'")
        .replace(".d.cts\"", ".cjs\"");
      result.push_str(&fixed);
    }
    result.push('\n');
  }
  // Remove trailing newline if original didn't have one
  if !code.ends_with('\n') && result.ends_with('\n') {
    result.pop();
  }
  result
}
