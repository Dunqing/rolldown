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
