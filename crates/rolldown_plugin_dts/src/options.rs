/// Override TypeScript `compilerOptions` relevant to DTS generation.
///
/// Mirrors the `compilerOptions` field from `rolldown-plugin-dts`.
#[derive(Debug, Clone, Default)]
pub struct DtsCompilerOptions {
  /// Whether to strip `@internal` annotated declarations from the output.
  ///
  /// Corresponds to TypeScript's `compilerOptions.stripInternal`.
  pub strip_internal: bool,
}

/// Options for the DTS plugin.
#[derive(Debug, Clone, Default)]
pub struct DtsPluginOptions {
  /// Whether to emit only `.d.ts` files (remove JS chunks from output).
  pub emit_dts_only: bool,
  /// If `true`, convert a single `export { x as default }` to `export = x`
  /// in the output for CommonJS compatibility.
  pub cjs_default: bool,
  /// Whether the generated `.d.ts` files have side effects.
  /// If `false` (default), Rolldown treats `.d.ts` files as side-effect-free during tree-shaking.
  pub side_effects: bool,
  /// Override TypeScript `compilerOptions` relevant to DTS generation.
  pub compiler_options: DtsCompilerOptions,
}
