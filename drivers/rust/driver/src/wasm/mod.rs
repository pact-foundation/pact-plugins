use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::anyhow;
use tracing::debug;
use wasmtime::{Engine, ExternRef, Instance, Module, Store};

use crate::catalogue_manager::{CatalogueEntry, register_plugin_entries};
use crate::plugin_manager::publish_updated_catalogue;
use crate::plugin_models::{PactPlugin, PactPluginManifest};

pub(crate) fn start_plugin_component(
  manifest: &PactPluginManifest,
  plugin_register: &mut HashMap<String, PactPlugin>
) -> anyhow::Result<PactPlugin> {
  debug!("Starting a WASM/WIT plugin");

  let engine = Engine::default();
  let mut wasm_path = PathBuf::from(&manifest.entry_point);
  if !wasm_path.is_absolute() || !wasm_path.exists() {
    wasm_path = PathBuf::from(manifest.plugin_dir.clone()).join(wasm_path);
  }
  let module = Module::from_file(&engine, wasm_path)?;
  let mut store = Store::new(&engine, ());
  let instance = Instance::new(&mut store, &module, &[])?;
  let init_function = instance.get_func(&mut store, "init-plugin")
    .ok_or_else(|| anyhow!("WASM module {} does not contain an init-plugin function", manifest.entry_point))?
    .typed::<(String, String), Result<Vec<ExternRef>, ()>>(&store)?;

  let result = init_function.call(&mut store, &["plugin-driver-rust".to_string(),
    option_env!("CARGO_PKG_VERSION").unwrap_or("0").to_string()])??;
  debug!("Got init response {:?} from plugin {}", result, manifest.name);

  register_plugin_entries(manifest, &vec![]);
  tokio::task::spawn(publish_updated_catalogue());

  todo!()
}
