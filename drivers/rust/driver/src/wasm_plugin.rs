use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::anyhow;
use async_trait::async_trait;
use bytes::Bytes;
use itertools::Itertools;
use pact_models::bodies::OptionalBody;
use pact_models::content_types::ContentType;
use pact_models::pact::Pact;
use pact_models::prelude::v4::V4Pact;
use pact_models::v4::interaction::V4Interaction;
use serde_json::Value;
use tracing::{debug, info, trace};
use wasmtime::{AsContextMut, Engine, Module, Store};
use wasmtime::component::{bindgen, Component, Instance, Linker, ResourceTable};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

use crate::catalogue_manager::{CatalogueEntryProviderType, CatalogueEntryType, register_plugin_entries};
use crate::mock_server::{MockServerConfig, MockServerDetails, MockServerResults};
use crate::plugin_models::{
  CompareContentRequest,
  CompareContentResult,
  GenerateContentRequest,
  PactPlugin,
  PactPluginManifest,
  PluginInteractionConfig
};
use crate::verification::{InteractionVerificationData, InteractionVerificationResult};

bindgen!();

impl Into<CatalogueEntryType> for EntryType {
  fn into(self) -> CatalogueEntryType {
    match self {
      EntryType::ContentMatcher => CatalogueEntryType::CONTENT_MATCHER,
      EntryType::ContentGenerator => CatalogueEntryType::CONTENT_GENERATOR,
      EntryType::Transport => CatalogueEntryType::TRANSPORT,
      EntryType::Matcher => CatalogueEntryType::MATCHER,
      EntryType::Interaction => CatalogueEntryType::INTERACTION
    }
  }
}

impl From<CatalogueEntryType> for EntryType {
  fn from(value: CatalogueEntryType) -> Self {
    match value {
      CatalogueEntryType::CONTENT_MATCHER => EntryType::ContentMatcher,
      CatalogueEntryType::CONTENT_GENERATOR => EntryType::ContentGenerator,
      CatalogueEntryType::TRANSPORT => EntryType::Transport,
      CatalogueEntryType::MATCHER => EntryType::Matcher,
      CatalogueEntryType::INTERACTION => EntryType::Interaction
    }
  }
}

impl From<&crate::catalogue_manager::CatalogueEntry> for CatalogueEntry {
  fn from(entry: &crate::catalogue_manager::CatalogueEntry) -> Self {
    CatalogueEntry {
      entry_type: entry.entry_type.into(),
      key: entry.key.clone(),
      values: entry.values.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
  }
}

impl Into<pact_models::content_types::ContentTypeHint> for ContentTypeHint {
  fn into(self) -> pact_models::content_types::ContentTypeHint {
    match self {
      ContentTypeHint::Binary => pact_models::content_types::ContentTypeHint::BINARY,
      ContentTypeHint::Text => pact_models::content_types::ContentTypeHint::TEXT,
      ContentTypeHint::Default => pact_models::content_types::ContentTypeHint::DEFAULT
    }
  }
}

impl From<CompareContentRequest> for CompareContentsRequest {
  fn from(value: CompareContentRequest) -> Self {
    CompareContentsRequest {
      expected: value.expected_contents.into(),
      actual: value.actual_contents.into(),
      allow_unexpected_keys: value.allow_unexpected_keys,
      plugin_configuration: value.plugin_configuration
        .map(|config| config.into())
        .unwrap_or_else(|| PluginConfiguration {
          interaction_configuration: Default::default(),
          pact_configuration: Default::default()
        })
    }
  }
}

impl From<OptionalBody>  for Body {
  fn from(value: OptionalBody) -> Self {
    Body {
      content: value.value().unwrap_or_default().to_vec(),
      content_type: value.content_type().unwrap_or_default().to_string(),
      content_type_hint: None
    }
  }
}

impl From<PluginInteractionConfig> for PluginConfiguration {
  fn from(value: PluginInteractionConfig) -> Self {
    PluginConfiguration {
      pact_configuration: Value::Object(value.pact_configuration
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()).to_string(),
      interaction_configuration: Value::Object(value.interaction_configuration
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()).to_string(),
    }
  }
}

impl Into<CompareContentResult> for CompareContentsResponse {
  fn into(self) -> CompareContentResult {
    if let Some(type_mismatch) = &self.type_mismatch {
      CompareContentResult::TypeMismatch(type_mismatch.expected.clone(), type_mismatch.actual.clone())
    } else if !self.results.is_empty() {
      let mismatches = self.results
        .iter()
        .map(|(path, mismatches)| {
          (path.clone(), mismatches.iter().map(|m| m.into()).collect())
        })
        .collect();
      CompareContentResult::Mismatches(mismatches)
    } else {
      CompareContentResult::OK
    }
  }
}

impl From<&ContentMismatch> for crate::content::ContentMismatch {
  fn from(value: &ContentMismatch) -> Self {
    crate::content::ContentMismatch {
      expected: "".to_string(),
      actual: "".to_string(),
      mismatch: value.mismatch.to_string(),
      path: value.path.to_string(),
      diff: None,
      mismatch_type: None,
    }
  }
}

/// Plugin that executes in a WASM VM
#[derive(Clone)]
pub struct WasmPlugin {
  manifest: PactPluginManifest,
  engine: Engine,
  component: Component,
  instance: Arc<Plugin>,
  store: Arc<Mutex<Store<PluginState>>>,
  access_count: Arc<AtomicUsize>
}

#[async_trait]
impl PactPlugin for WasmPlugin {
  fn manifest(&self) -> PactPluginManifest {
    self.manifest.clone()
  }

  fn kill(&self) {
    // TODO: work out how to shut the WASM instance down
  }

  fn update_access(&mut self) {
    let count = self.access_count.fetch_add(1, Ordering::SeqCst);
    trace!("update_access: Plugin {}/{} access is now {}", self.manifest.name,
      self.manifest.version, count + 1);
  }

  fn drop_access(&mut self) -> usize {
    let check = self.access_count.fetch_update(Ordering::SeqCst,
      Ordering::SeqCst, |count| {
        if count > 0 {
          Some(count - 1)
        } else {
          None
        }
      });
    let count = if let Ok(v) = check {
      if v > 0 { v - 1 } else { v }
    } else {
      0
    };
    trace!("drop_access: Plugin {}/{} access is now {}", self.manifest.name, self.manifest.version,
      count);
    count
  }

  fn boxed(&self) -> Box<dyn PactPlugin + Send + Sync> {
    Box::new(self.clone())
  }

  fn arced(&self) -> Arc<dyn PactPlugin + Send + Sync> {
    Arc::new(self.clone())
  }

  async fn publish_updated_catalogue(&self, catalogue: &[crate::catalogue_manager::CatalogueEntry]) -> anyhow::Result<()> {
    let mut store = self.store.lock().unwrap();
    let catalogue = catalogue.iter()
      .map(|entry| entry.into())
      .collect_vec();
    self.instance.call_update_catalogue(store.as_context_mut(), &catalogue)
  }

  async fn generate_contents(&self, request: GenerateContentRequest) -> anyhow::Result<OptionalBody> {
    todo!()
  }

  async fn match_contents(
    &self,
    request: CompareContentRequest
  ) -> anyhow::Result<CompareContentResult> {
    let mut store = self.store.lock().unwrap();

    let result = self.instance.call_compare_contents(store.as_context_mut(), &request.into())?
      .map_err(|err| anyhow!(err))?;
    debug!("Result from call to compare_contents: {:?}", result);

    Ok(result.into())
  }

  async fn configure_interaction(
    &self,
    content_type: &ContentType,
    definition: &HashMap<String, Value>
  ) -> anyhow::Result<(Vec<crate::content::InteractionContents>, Option<crate::content::PluginConfiguration>)> {
    let mut store = self.store.lock().unwrap();

    // Unfortunately, WIT does not allow recursive data structures, so we need to feed JSON strings
    // in and out here.
    let json = Value::Object(definition.iter()
      .map(|(k, v)| (k.clone(), v.clone()))
      .collect());
    let result = self.instance.call_configure_interaction(store.as_context_mut(),
      content_type.sub_type.to_string().as_str(), json.to_string().as_str())?
      .map_err(|err| anyhow!(err))?;
    debug!("Result from call to configure_interaction: {:?}", result);

    let mut interaction_details = vec![];
    for config in &result.interaction {
      interaction_details.push(crate::content::InteractionContents {
        part_name: config.part_name.clone(),
        body: OptionalBody::Present(
          Bytes::copy_from_slice(config.contents.content.as_slice()),
          ContentType::parse(config.contents.content_type.as_str()).ok(),
          config.contents.content_type_hint.map(|v| v.into())
        ),
        rules: None,
        generators: None,
        metadata: None,
        metadata_rules: None,
        plugin_config: Default::default(),
        interaction_markup: "".to_string(),
        interaction_markup_type: "".to_string(),
      });
    }
    debug!("interaction_details = {:?}", interaction_details);

    let plugin_config = match result.plugin_config {
      Some(config) => {
        let interaction_configuration = if config.interaction_configuration.is_empty() {
          Default::default()
        } else {
          serde_json::from_str::<Value>(config.interaction_configuration.as_str())?
            .as_object()
            .cloned()
            .unwrap_or_default()
        };
        let pact_configuration = if config.pact_configuration.is_empty() {
          Default::default()
        } else {
          serde_json::from_str::<Value>(config.pact_configuration.as_str())?
            .as_object()
            .cloned()
            .unwrap_or_default()
        };
        Some(crate::content::PluginConfiguration {
          interaction_configuration: interaction_configuration.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
          pact_configuration: pact_configuration.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        })
      }
      None => None
    };
    debug!("plugin_config = {:?}", plugin_config);

    Ok((interaction_details, plugin_config))
  }

  async fn verify_interaction(&self, pact: &V4Pact, interaction: &(dyn V4Interaction + Send + Sync), verification_data: &InteractionVerificationData, config: &HashMap<String, Value>) -> anyhow::Result<InteractionVerificationResult> {
    todo!()
  }

  async fn prepare_interaction_for_verification(&self, pact: &V4Pact, interaction: &(dyn V4Interaction + Send + Sync), context: &HashMap<String, Value>) -> anyhow::Result<InteractionVerificationData> {
    todo!()
  }

  async fn start_mock_server(&self, config: &MockServerConfig, pact: Box<dyn Pact + Send + Sync>, test_context: HashMap<String, Value>) -> anyhow::Result<MockServerDetails> {
    todo!()
  }

  async fn get_mock_server_results(&self, mock_server_key: &str) -> anyhow::Result<Vec<MockServerResults>> {
    todo!()
  }

  async fn shutdown_mock_server(&self, mock_server_key: &str) -> anyhow::Result<Vec<MockServerResults>> {
    todo!()
  }
}

impl Debug for WasmPlugin {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("WasmPlugin")
      .field("manifest", &self.manifest)
      .finish()
  }
}

impl WasmPlugin {
  /// Calls the plugins init function
  pub fn init(&self) -> anyhow::Result<()> {
    let result = {
      let mut store = self.store.lock().unwrap();
      self.instance.call_init(store.as_context_mut(), "plugin-driver-rust", option_env!("CARGO_PKG_VERSION")
        .unwrap_or("0"))?
    };

    debug!("Got the following entries from the plugin: {:?}", result);
    register_plugin_entries(&self.manifest, result.iter()
      .map(|v| {
        crate::catalogue_manager::CatalogueEntry {
          entry_type: v.entry_type.into(),
          provider_type: CatalogueEntryProviderType::PLUGIN,
          plugin: Some(self.manifest.clone()),
          key: v.key.clone(),
          values: v.values.iter().cloned().collect()
        }
      })
      .collect()
    );

    Ok(())
  }
}

struct PluginState {
  table: ResourceTable,
  ctx: WasiCtx,
  plugin_name: String
}

impl WasiView for PluginState {
  fn table(&mut self) -> &mut ResourceTable {
    &mut self.table
  }

  fn ctx(&mut self) -> &mut WasiCtx {
    &mut self.ctx
  }
}

impl PluginImports for PluginState {
  fn log(&mut self, message: String) {
    debug!("Plugin({}) || {}", self.plugin_name, message);
  }
}

// Loads and initialises a WASM-based plugin
pub fn load_wasm_plugin(manifest: &PactPluginManifest) -> anyhow::Result<WasmPlugin> {
  let engine = Engine::default();

  let mut path = PathBuf::from(&manifest.entry_point);
  if !path.is_absolute() || !path.exists() {
    path = PathBuf::from(manifest.plugin_dir.clone()).join(path);
  }
  debug!("Loading plugin component from path {:?}", &path);
  let component = Component::from_file(&engine, path)?;

  let mut linker = Linker::new(&engine);
  wasmtime_wasi::add_to_linker_sync(&mut linker)?;
  Plugin::add_to_linker(&mut linker, |state: &mut PluginState| state)?;

  let mut store = Store::new(&engine, PluginState {
    table: Default::default(),
    ctx: WasiCtxBuilder::new().build(),
    plugin_name: format!("{}/{}", manifest.name, manifest.version),
  });
  let instance = Plugin::instantiate(&mut store, &component, &linker)?;

  let plugin = WasmPlugin {
    manifest: manifest.clone(),
    engine,
    component,
    instance: Arc::new(instance),
    store: Arc::new(Mutex::new(store)),
    access_count: Arc::new(AtomicUsize::new(1))
  };

  Ok(plugin)
}
