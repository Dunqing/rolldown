use std::path::Path;

/// File extensions that can produce `.d.ts` declarations.
const DTS_PRODUCIBLE_EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts"];

/// Check if a file ID is a TypeScript source file that can produce declarations.
/// Excludes `.d.ts` / `.d.mts` / `.d.cts` files which are already declarations.
pub fn is_ts_source(id: &str) -> bool {
  // Exclude declaration files first
  if is_dts(id) {
    return false;
  }
  let path = Path::new(id);
  path
    .extension()
    .and_then(|ext| ext.to_str())
    .is_some_and(|ext| DTS_PRODUCIBLE_EXTENSIONS.contains(&ext))
}

/// Check if a file ID is a `.d.ts` declaration file.
pub fn is_dts(id: &str) -> bool {
  id.ends_with(".d.ts") || id.ends_with(".d.mts") || id.ends_with(".d.cts")
}

/// Convert a source file path to its corresponding `.d.ts` path.
/// e.g., `foo.ts` -> `foo.d.ts`, `bar.tsx` -> `bar.d.ts`
pub fn source_to_dts(id: &str) -> String {
  let path = Path::new(id);
  let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
  let parent = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
  if parent.is_empty() { format!("{stem}.d.ts") } else { format!("{parent}/{stem}.d.ts") }
}

/// Convert a `.d.ts` path back to a virtual module ID prefix.
/// We use `\0dts:` prefix for virtual DTS modules.
pub fn make_dts_virtual_id(source_id: &str) -> String {
  format!("\0dts:{}", source_to_dts(source_id))
}

/// Check if a module ID is a virtual DTS module.
pub fn is_dts_virtual_id(id: &str) -> bool {
  id.starts_with("\0dts:")
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Extract the real `.d.ts` path from a virtual DTS module ID.
  fn virtual_id_to_dts_path(id: &str) -> &str {
    id.strip_prefix("\0dts:").unwrap_or(id)
  }

  /// Convert a `.d.ts` import path to its `.js` counterpart for output.
  /// e.g., `./foo.d.ts` -> `./foo.js`
  fn dts_to_js_extension(id: &str) -> String {
    if let Some(stripped) = id.strip_suffix(".d.ts") {
      format!("{stripped}.js")
    } else if let Some(stripped) = id.strip_suffix(".d.mts") {
      format!("{stripped}.mjs")
    } else if let Some(stripped) = id.strip_suffix(".d.cts") {
      format!("{stripped}.cjs")
    } else {
      id.to_string()
    }
  }

  #[test]
  fn test_is_ts_source() {
    assert!(is_ts_source("foo.ts"));
    assert!(is_ts_source("bar.tsx"));
    assert!(is_ts_source("baz.mts"));
    assert!(is_ts_source("qux.cts"));
    assert!(!is_ts_source("foo.js"));
    assert!(!is_ts_source("foo.d.ts"));
  }

  #[test]
  fn test_is_dts() {
    assert!(is_dts("foo.d.ts"));
    assert!(is_dts("bar.d.mts"));
    assert!(is_dts("baz.d.cts"));
    assert!(!is_dts("foo.ts"));
    assert!(!is_dts("foo.js"));
  }

  #[test]
  fn test_source_to_dts() {
    assert_eq!(source_to_dts("foo.ts"), "foo.d.ts");
    assert_eq!(source_to_dts("src/bar.tsx"), "src/bar.d.ts");
    assert_eq!(source_to_dts("/abs/path/baz.mts"), "/abs/path/baz.d.ts");
  }

  #[test]
  fn test_virtual_ids() {
    let virtual_id = make_dts_virtual_id("src/foo.ts");
    assert!(is_dts_virtual_id(&virtual_id));
    assert_eq!(virtual_id_to_dts_path(&virtual_id), "src/foo.d.ts");
  }

  #[test]
  fn test_dts_to_js_extension() {
    assert_eq!(dts_to_js_extension("./foo.d.ts"), "./foo.js");
    assert_eq!(dts_to_js_extension("./bar.d.mts"), "./bar.mjs");
    assert_eq!(dts_to_js_extension("./baz.d.cts"), "./baz.cjs");
    assert_eq!(dts_to_js_extension("./qux.js"), "./qux.js");
  }
}
