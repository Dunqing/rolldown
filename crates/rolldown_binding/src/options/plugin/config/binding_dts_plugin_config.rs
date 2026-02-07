use rolldown_plugin_dts::DtsPluginOptions;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingDtsPluginConfig {
  /// Whether to strip `@internal` annotated declarations.
  pub strip_internal: Option<bool>,
  /// Whether to emit only `.d.ts` files (remove JS chunks from output).
  pub emit_dts_only: Option<bool>,
  /// Whether to generate source maps for declaration files.
  pub sourcemap: Option<bool>,
}

impl From<BindingDtsPluginConfig> for DtsPluginOptions {
  fn from(config: BindingDtsPluginConfig) -> Self {
    Self {
      strip_internal: config.strip_internal.unwrap_or_default(),
      emit_dts_only: config.emit_dts_only.unwrap_or_default(),
      sourcemap: config.sourcemap.unwrap_or_default(),
    }
  }
}
