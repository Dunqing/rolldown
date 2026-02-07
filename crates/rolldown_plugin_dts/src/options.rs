/// Options for the DTS plugin.
#[derive(Debug, Clone, Default)]
pub struct DtsPluginOptions {
  /// Whether to strip `@internal` annotated declarations.
  pub strip_internal: bool,
  /// Whether to emit only `.d.ts` files (remove JS chunks from output).
  pub emit_dts_only: bool,
  /// Whether to generate source maps for declaration files.
  pub sourcemap: bool,
}
