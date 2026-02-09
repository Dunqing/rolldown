use rolldown_plugin_dts::{DtsCompilerOptions, DtsPluginOptions};

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingDtsCompilerOptions {
  /// Whether to strip `@internal` annotated declarations.
  /// Corresponds to TypeScript's `compilerOptions.stripInternal`.
  pub strip_internal: Option<bool>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingDtsPluginConfig {
  /// Whether to emit only `.d.ts` files (remove JS chunks from output).
  pub emit_dts_only: Option<bool>,
  /// If `true`, convert a single `export { x as default }` to `export = x`
  /// in the output for CommonJS compatibility.
  pub cjs_default: Option<bool>,
  /// Whether the generated `.d.ts` files have side effects.
  /// If `false` (default), Rolldown treats `.d.ts` files as side-effect-free during tree-shaking.
  pub side_effects: Option<bool>,
  /// Override TypeScript `compilerOptions` relevant to DTS generation.
  pub compiler_options: Option<BindingDtsCompilerOptions>,
}

impl From<BindingDtsPluginConfig> for DtsPluginOptions {
  fn from(config: BindingDtsPluginConfig) -> Self {
    let compiler_options = config.compiler_options.unwrap_or_default();
    Self {
      emit_dts_only: config.emit_dts_only.unwrap_or_default(),
      cjs_default: config.cjs_default.unwrap_or_default(),
      side_effects: config.side_effects.unwrap_or_default(),
      compiler_options: DtsCompilerOptions {
        strip_internal: compiler_options.strip_internal.unwrap_or_default(),
      },
    }
  }
}
