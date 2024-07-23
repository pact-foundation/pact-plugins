//! Manages interactions with Pact plugins
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{anyhow, bail, Context};
use lazy_static::lazy_static;
use maplit::hashmap;
use pact_models::json_utils::json_to_string;
use pact_models::prelude::Pact;
use pact_models::prelude::v4::V4Pact;
use pact_models::v4::interaction::V4Interaction;
use reqwest::Client;
use semver::Version;
use serde_json::Value;
use sysinfo::{Pid,System};
#[cfg(not(windows))] use sysinfo::Signal;
#[cfg(not(windows))] use tokio::process::Command;
use tracing::{debug, info, trace, warn};

use crate::catalogue_manager::{all_entries, CatalogueEntry, remove_plugin_entries};
use crate::download::{download_json_from_github, download_plugin_executable, fetch_json_from_url};
use crate::grpc_plugin::{init_handshake, start_plugin_process};
use crate::lua_plugin::start_lua_plugin;
use crate::metrics::send_metrics;
use crate::mock_server::{MockServerConfig, MockServerDetails, MockServerResults};
use crate::plugin_models::{PactPlugin, PactPluginManifest, PluginDependency};
use crate::repository::{fetch_repository_index, USER_AGENT};
use crate::utils::versions_compatible;
use crate::verification::{InteractionVerificationData, InteractionVerificationResult};
use crate::wasm_plugin::load_wasm_plugin;

lazy_static! {
  static ref PLUGIN_MANIFEST_REGISTER: Mutex<HashMap<String, PactPluginManifest>> = Mutex::new(HashMap::new());
  pub static ref PLUGIN_REGISTER: Mutex<HashMap<String, Arc<dyn PactPlugin + Send + Sync>>> = Mutex::new(HashMap::new());
}

/// Load the plugin defined by the dependency information. Will first look in the global
/// plugin registry.
pub async fn load_plugin<'a>(plugin: &PluginDependency) -> anyhow::Result<Arc<dyn PactPlugin + Send + Sync>> {
  let thread_id = thread::current().id();
  debug!("Loading plugin {:?}", plugin);
  trace!("Rust plugin driver version {}", option_env!("CARGO_PKG_VERSION").unwrap_or_default());

  let result = {
    trace!("load_plugin {:?}: Waiting on PLUGIN_REGISTER lock", thread_id);
    let mut inner = PLUGIN_REGISTER.lock().unwrap();
    trace!("load_plugin {:?}: Got PLUGIN_REGISTER lock", thread_id);
    let update_access = |plugin: &mut (dyn PactPlugin + Send + Sync)| {
      debug!("Found running plugin {:?}", plugin);
      plugin.update_access();
      plugin.arced()
    };
    let result = match with_plugin_mut(plugin, &mut inner, &update_access) {
      Some(plugin) => Ok((plugin, false)),
      None => {
        debug!("Did not find plugin, will attempt to start it");
        let manifest = match load_plugin_manifest(plugin) {
          Ok(manifest) => manifest,
          Err(err) => {
            warn!("Could not load plugin manifest from disk, will try auto install it: {}", err);
            let http_client = reqwest::ClientBuilder::new()
              .user_agent(USER_AGENT)
              .build()?;
            let index = fetch_repository_index(&http_client, None).await?;
            match index.lookup_plugin_version(&plugin.name, &plugin.version) {
              Some(entry) => {
                info!("Found an entry for the plugin in the plugin index, will try install that");
                install_plugin_from_url(&http_client, entry.source.value().as_str()).await?
              }
              None => Err(err)?
            }
          }
        };
        send_metrics(&manifest);
        initialise_plugin(&manifest, &mut inner).await
          .map(|plugin| (plugin, true))
      }
    };
    trace!("load_plugin {:?}: Releasing PLUGIN_REGISTER lock", thread_id);
    result
  };

  if let Ok((_, new_plugin)) = &result {
    if *new_plugin {
      publish_updated_catalogue();
    }
  }

  result.map(|(plugin, _)| plugin)
}

fn lookup_plugin_inner(
  plugin: &PluginDependency,
  plugin_register: &HashMap<String, Arc<dyn PactPlugin + Send + Sync>>
) -> Option<Arc<dyn PactPlugin + Send + Sync>> {
  if let Some(version) = &plugin.version {
    plugin_register.get(format!("{}/{}", plugin.name, version).as_str())
      .map(|plugin| plugin.clone())
  } else {
    plugin_register.iter()
      .filter(|(_, value)| value.manifest().name == plugin.name)
      .max_by(|(_, v1), (_, v2)| v1.manifest().version.cmp(&v2.manifest().version))
      .map(|(_, plugin)| plugin.clone())
  }
}

fn with_plugin_mut<R>(
  plugin: &PluginDependency,
  plugin_register: &mut HashMap<String, Arc<dyn PactPlugin + Send + Sync>>,
  f: &dyn Fn(&mut (dyn PactPlugin + Send + Sync)) -> R
) -> Option<R> {
  if let Some(version) = &plugin.version {
    plugin_register.get_mut(format!("{}/{}", plugin.name, version).as_str())
      .map(|plugin| Arc::get_mut(plugin).map(|inner| f(inner)))
      .flatten()
  } else {
    plugin_register.iter_mut()
      .filter(|(_, value)| value.manifest().name == plugin.name)
      .max_by(|(_, v1), (_, v2)| v1.manifest().version.cmp(&v2.manifest().version))
      .map(|(_, plugin)| Arc::get_mut(plugin).map(|inner| f(inner)))
      .flatten()
  }
}

/// Look up the plugin in the global plugin register
pub fn lookup_plugin<'a>(plugin: &PluginDependency) -> Option<Arc<dyn PactPlugin + Send + Sync>> {
  let thread_id = thread::current().id();
  trace!("lookup_plugin {:?}: Waiting on PLUGIN_REGISTER lock", thread_id);
  let mut inner = PLUGIN_REGISTER.lock().unwrap();
  trace!("lookup_plugin {:?}: Got PLUGIN_REGISTER lock", thread_id);
  let entry = lookup_plugin_inner(plugin, &mut inner);
  trace!("lookup_plugin {:?}: Releasing PLUGIN_REGISTER lock", thread_id);
  entry
}

/// Return the plugin manifest for the given plugin. Will first look in the global plugin manifest
/// registry.
pub fn load_plugin_manifest(plugin_dep: &PluginDependency) -> anyhow::Result<PactPluginManifest> {
  debug!("Loading plugin manifest for plugin {:?}", plugin_dep);
  match lookup_plugin_manifest(plugin_dep) {
    Some(manifest) => Ok(manifest),
    None => load_manifest_from_disk(plugin_dep)
  }
}

fn load_manifest_from_disk(plugin_dep: &PluginDependency) -> anyhow::Result<PactPluginManifest> {
  let plugin_dir = pact_plugin_dir()?;
  debug!("Looking for plugin in {:?}", plugin_dir);

  if plugin_dir.exists() {
    load_manifest_from_dir(plugin_dep, &plugin_dir)
  } else {
    Err(anyhow!("Plugin directory {:?} does not exist", plugin_dir))
  }
}

fn load_manifest_from_dir(plugin_dep: &PluginDependency, plugin_dir: &PathBuf) -> anyhow::Result<PactPluginManifest> {
  let mut manifests = vec![];
  for entry in fs::read_dir(plugin_dir)? {
    let path = entry?.path();
    trace!("Found: {:?}", path);

    if path.is_dir() {
      let manifest_file = path.join("pact-plugin.json");
      if manifest_file.exists() && manifest_file.is_file() {
        debug!("Found plugin manifest: {:?}", manifest_file);
        let file = File::open(manifest_file)?;
        let reader = BufReader::new(file);
        let manifest: PactPluginManifest = serde_json::from_reader(reader)?;
        trace!("Parsed plugin manifest: {:?}", manifest);
        let version = manifest.version.clone();
        if manifest.name == plugin_dep.name && versions_compatible(version.as_str(), &plugin_dep.version) {
          let manifest = PactPluginManifest {
            plugin_dir: path.to_string_lossy().to_string(),
            ..manifest
          };
          manifests.push(manifest);
        }
      }
    }
  }

  let manifest = manifests.iter()
    .max_by(|a, b| {
      let a = Version::parse(&a.version).unwrap_or_else(|_| Version::new(0, 0, 0));
      let b = Version::parse(&b.version).unwrap_or_else(|_| Version::new(0, 0, 0));
      a.cmp(&b)
    });
  if let Some(manifest) = manifest {
    let key = format!("{}/{}", manifest.name, manifest.version);
    {
      let mut guard = PLUGIN_MANIFEST_REGISTER.lock().unwrap();
      guard.insert(key.clone(), manifest.clone());
    }
    Ok(manifest.clone())
  } else {
    Err(anyhow!("Plugin {} was not found (in $HOME/.pact/plugins or $PACT_PLUGIN_DIR)", plugin_dep))
  }
}

pub(crate) fn pact_plugin_dir() -> anyhow::Result<PathBuf> {
  let env_var = env::var_os("PACT_PLUGIN_DIR");
  let plugin_dir = env_var.unwrap_or_default();
  let plugin_dir = plugin_dir.to_string_lossy();
  if plugin_dir.is_empty() {
    home::home_dir().map(|dir| dir.join(".pact").join("plugins"))
  } else {
    PathBuf::from_str(plugin_dir.as_ref()).ok()
  }.ok_or_else(|| anyhow!("No Pact plugin directory was found (in $HOME/.pact/plugins or $PACT_PLUGIN_DIR)"))
}

/// Lookup the plugin manifest in the global plugin manifest registry.
pub fn lookup_plugin_manifest(plugin: &PluginDependency) -> Option<PactPluginManifest> {
  let guard = PLUGIN_MANIFEST_REGISTER.lock().unwrap();
  if let Some(version) = &plugin.version {
    let key = format!("{}/{}", plugin.name, version);
    guard.get(&key).cloned()
  } else {
    guard.iter()
      .filter(|(_, value)| value.name == plugin.name)
      .max_by(|(_, v1), (_, v2)| v1.version.cmp(&v2.version))
      .map(|(_, p)| p.clone())
  }
}

async fn initialise_plugin<'a>(
  manifest: &PactPluginManifest,
  plugin_register: &'a mut HashMap<String, Arc<dyn PactPlugin + Send + Sync>>
) -> anyhow::Result<Arc<dyn PactPlugin + Send + Sync>> {
  match manifest.executable_type.as_str() {
    "exec" => {
      let mut plugin = start_plugin_process(manifest).await?;
      debug!("Plugin process started OK (port = {}), sending init message", plugin.port());

      init_handshake(manifest, &mut plugin).await.map_err(|err| {
        plugin.kill();
        anyhow!("Failed to send init request to the plugin - {}", err)
      })?;

      let arc = Arc::new(plugin);
      let key = format!("{}/{}", manifest.name, manifest.version);
      plugin_register.insert(key, arc.clone());

      Ok(arc)
    }
    "lua" => {
      #[cfg(feature = "lua")] {
        let plugin = start_lua_plugin(manifest)?;
        debug!("Plugin started OK ({:?}), sending init message", plugin);

        plugin.init()?;

        let arc = Arc::new(plugin);
        let key = format!("{}/{}", manifest.name, manifest.version);
        plugin_register.insert(key, arc.clone());

        Ok(arc)
      }
      #[cfg(not(feature = "lua"))] {
        Err(anyhow!("Lua plugins are not supported (Lua feature flag is not enabled)"))
      }
    }
    "wasm" => {
      #[cfg(feature = "wasm")] {
        let plugin = load_wasm_plugin(manifest)?;
        debug!("Plugin loaded OK ({:?}), sending init message", plugin);

        plugin.init()?;

        let arc = Arc::new(plugin);
        let key = format!("{}/{}", manifest.name, manifest.version);
        plugin_register.insert(key, arc.clone());

        Ok(arc)
      }
      #[cfg(not(feature = "wasm"))] {
        Err(anyhow!("WASM plugins are not supported (wasm feature flag is not enabled)"))
      }
    }
    _ => Err(anyhow!("Plugin executable type of {} is not supported", manifest.executable_type))
  }
}

/// Shut down all plugin processes
pub fn shutdown_plugins() {
  let thread_id = thread::current().id();
  debug!("Shutting down all plugins");
  trace!("shutdown_plugins {:?}: Waiting on PLUGIN_REGISTER lock", thread_id);
  let mut guard = crate::plugin_manager::PLUGIN_REGISTER.lock().unwrap();
  trace!("shutdown_plugins {:?}: Got PLUGIN_REGISTER lock", thread_id);
  for plugin in guard.values() {
    debug!("Shutting down plugin {:?}", plugin);
    plugin.kill();
    remove_plugin_entries(&plugin.manifest().name);
  }
  guard.clear();
  trace!("shutdown_plugins {:?}: Releasing PLUGIN_REGISTER lock", thread_id);
}

/// Shutdown the given plugin
pub fn shutdown_plugin(plugin: &(dyn PactPlugin + Send + Sync)) {
  debug!("Shutting down plugin {}:{}", plugin.manifest().name, plugin.manifest().version);
  plugin.kill();
  remove_plugin_entries(&plugin.manifest().name);
}

/// Publish the current catalogue to all plugins
pub fn publish_updated_catalogue() {
  let thread_id = thread::current().id();
  let catalogue = all_entries();

  trace!("publish_updated_catalogue {:?}: Waiting on PLUGIN_REGISTER lock", thread_id);
  let inner = PLUGIN_REGISTER.lock().unwrap();
  trace!("publish_updated_catalogue {:?}: Got PLUGIN_REGISTER lock", thread_id);
  for plugin in inner.values() {
    publish_catalogue_to_plugin(catalogue.clone(), plugin.clone());
  }

  trace!("publish_updated_catalogue {:?}: Releasing PLUGIN_REGISTER lock", thread_id);
}

fn publish_catalogue_to_plugin(catalogue: Vec<CatalogueEntry>, plugin: Arc<dyn PactPlugin + Send + Sync>) {
  tokio::task::spawn(async move {
    if let Err(err) = plugin.publish_updated_catalogue(catalogue.as_slice()).await {
      warn!("Failed to send updated catalogue to plugin '{}' - {}", plugin.manifest().name, err);
    }
  });
}

/// Increment access to the plugin.
#[tracing::instrument]
pub fn increment_plugin_access(plugin_dep: &PluginDependency) {
  let thread_id = thread::current().id();

  trace!("increment_plugin_access {:?}: Waiting on PLUGIN_REGISTER lock", thread_id);
  let mut inner = PLUGIN_REGISTER.lock().unwrap();
  trace!("increment_plugin_access {:?}: Got PLUGIN_REGISTER lock", thread_id);

  if with_plugin_mut(plugin_dep, &mut inner, &|plugin| plugin.update_access()).is_none() {
    warn!("Plugin {} was not found", plugin_dep);
  }

  trace!("increment_plugin_access {:?}: Releasing PLUGIN_REGISTER lock", thread_id);
}

/// Decrement access to the plugin. If the current access count is zero, shut down the plugin
#[tracing::instrument]
pub fn drop_plugin_access(plugin_dep: &PluginDependency) {
  let thread_id = thread::current().id();

  trace!("drop_plugin_access {:?}: Waiting on PLUGIN_REGISTER lock", thread_id);
  let mut inner = PLUGIN_REGISTER.lock().unwrap();
  trace!("drop_plugin_access {:?}: Got PLUGIN_REGISTER lock", thread_id);

  match with_plugin_mut(plugin_dep, &mut inner, &|plugin| {
    if plugin.drop_access() == 0 {
      shutdown_plugin(plugin);
      Some(format!("{}/{}", plugin.manifest().name, plugin.manifest().version))
    } else {
      None
    }
  }) {
    Some(dropped) => {
      if let Some(key) = dropped {
        inner.remove(key.as_str());
      }
    }
    None => warn!("Plugin {} was not found", plugin_dep)
  }

  trace!("drop_plugin_access {:?}: Releasing PLUGIN_REGISTER lock", thread_id);
}

/// Starts a mock server given the catalog entry for it and a Pact
#[deprecated(note = "Use start_mock_server_v2 which takes a test context map", since = "0.2.2")]
pub async fn start_mock_server(
  catalogue_entry: &CatalogueEntry,
  pact: Box<dyn Pact + Send + Sync>,
  config: MockServerConfig
) -> anyhow::Result<MockServerDetails> {
  start_mock_server_v2(catalogue_entry, pact, config, hashmap!{}).await
}

/// Starts a mock server given the catalog entry for it and a Pact
pub async fn start_mock_server_v2(
  catalogue_entry: &CatalogueEntry,
  pact: Box<dyn Pact + Send + Sync>,
  config: MockServerConfig,
  test_context: HashMap<String, Value>
) -> anyhow::Result<MockServerDetails> {
  let manifest = catalogue_entry.plugin.as_ref()
    .ok_or_else(|| anyhow!("Catalogue entry did not have an associated plugin manifest"))?;
  let plugin = lookup_plugin(&manifest.as_dependency())
    .ok_or_else(|| anyhow!("Did not find a running plugin for manifest {:?}", manifest))?;

  debug!(plugin_name = manifest.name.as_str(), plugin_version = manifest.version.as_str(),
    "Sending startMockServer request to plugin");
  let response = plugin.as_ref().start_mock_server(
    &config,
    pact,
    test_context
  ).await;
  debug!("Got response ${response:?}");
  response
}

/// Shutdowns a running mock server. Will return any errors from the mock server.
pub async fn shutdown_mock_server(mock_server: &MockServerDetails) -> anyhow::Result<Vec<MockServerResults>> {
  debug!(
    plugin_name = mock_server.plugin.manifest().name.as_str(),
    plugin_version = mock_server.plugin.manifest().version.as_str(),
    server_key = mock_server.key.as_str(),
    "Sending shutdownMockServer request to plugin"
  );
  let response = mock_server.plugin.shutdown_mock_server(mock_server.key.as_str()).await;
  debug!("Got response: {response:?}");
  response
}

/// Gets the results from a running mock server.
pub async fn get_mock_server_results(mock_server: &MockServerDetails) -> anyhow::Result<Vec<MockServerResults>> {
  debug!(
    plugin_name = mock_server.plugin.manifest().name.as_str(),
    plugin_version = mock_server.plugin.manifest().version.as_str(),
    server_key = mock_server.key.as_str(),
    "Sending getMockServerResults request to plugin"
  );
  let response = mock_server.plugin.get_mock_server_results(mock_server.key.as_str()).await;
  debug!("Got response: {response:?}");
  response
}

/// Sets up a transport request to be made. This is the first phase when verifying, and it allows the
/// users to add additional values to any requests that are made.
pub async fn prepare_validation_for_interaction(
  transport_entry: &CatalogueEntry,
  pact: &V4Pact,
  interaction: &(dyn V4Interaction + Send + Sync),
  context: &HashMap<String, Value>
) -> anyhow::Result<InteractionVerificationData> {
  let manifest = transport_entry.plugin.as_ref()
    .ok_or_else(|| anyhow!("Transport catalogue entry did not have an associated plugin manifest"))?;
  let plugin = lookup_plugin(&manifest.as_dependency())
    .ok_or_else(|| anyhow!("Did not find a running plugin for manifest {:?}", manifest))?;

  prepare_verification_for_interaction_inner(plugin.as_ref(), manifest, pact, interaction, context).await
}

pub(crate) async fn prepare_verification_for_interaction_inner(
  plugin: &(dyn PactPlugin + Send + Sync),
  manifest: &PactPluginManifest,
  pact: &V4Pact,
  interaction: &(dyn V4Interaction + Send + Sync),
  context: &HashMap<String, Value>
) -> anyhow::Result<InteractionVerificationData> {
  let mut pact = pact.clone();
  pact.interactions = pact.interactions.iter().map(|i| i.with_unique_key()).collect();

  debug!(plugin_name = manifest.name.as_str(), plugin_version = manifest.version.as_str(),
    "Sending prepare verification for interaction request to plugin");
  let response = plugin.prepare_interaction_for_verification(&pact, interaction, context).await;
  debug!("Got response: {response:?}");

  response
}

/// Executes the verification of the interaction that was configured with the prepare_validation_for_interaction call
pub async fn verify_interaction(
  transport_entry: &CatalogueEntry,
  verification_data: &InteractionVerificationData,
  config: &HashMap<String, Value>,
  pact: &V4Pact,
  interaction: &(dyn V4Interaction + Send + Sync)
) -> anyhow::Result<InteractionVerificationResult> {
  let manifest = transport_entry.plugin.as_ref()
    .ok_or_else(|| anyhow!("Transport catalogue entry did not have an associated plugin manifest"))?;
  let plugin = lookup_plugin(&manifest.as_dependency())
    .ok_or_else(|| anyhow!("Did not find a running plugin for manifest {:?}", manifest))?;

  verify_interaction_inner(
    plugin.as_ref(),
    &manifest,
    verification_data,
    config,
    pact,
    interaction
  ).await
}

pub(crate) async fn verify_interaction_inner(
  plugin: &(dyn PactPlugin + Send + Sync),
  manifest: &PactPluginManifest,
  verification_data: &InteractionVerificationData,
  config: &HashMap<String, Value>,
  pact: &V4Pact,
  interaction: &(dyn V4Interaction + Send + Sync)
) -> anyhow::Result<InteractionVerificationResult> {
  let mut pact = pact.clone();
  pact.interactions = pact.interactions.iter().map(|i| i.with_unique_key()).collect();

  debug!(plugin_name = manifest.name.as_str(), plugin_version = manifest.version.as_str(),
    "Sending verifyInteraction request to plugin");
  let response = plugin.verify_interaction(&pact, interaction, verification_data, config).await;
  debug!("Got response: {response:?}");
  response
}

/// Tries to download and install the plugin from the given URL, returning the manifest for the
/// plugin if successful.
pub async fn install_plugin_from_url(
  http_client: &Client,
  source_url: &str
) -> anyhow::Result<PactPluginManifest> {
  let response = fetch_json_from_url(source_url, http_client).await?;
  if let Some(map) = response.as_object() {
    if let Some(tag) = map.get("tag_name") {
      let tag = json_to_string(tag);
      debug!(%tag, "Found tag");
      let url = if source_url.ends_with("/latest") {
        source_url.strip_suffix("/latest").unwrap_or(source_url)
      } else {
        let suffix = format!("/tag/{}", tag);
        source_url.strip_suffix(suffix.as_str()).unwrap_or(source_url)
      };
      let manifest_json = download_json_from_github(&http_client, url, &tag, "pact-plugin.json")
        .await.context("Downloading manifest file from GitHub")?;
      let manifest: PactPluginManifest = serde_json::from_value(manifest_json)
        .context("Failed to parsing JSON manifest file from GitHub")?;
      debug!(?manifest, "Loaded manifest from GitHub");

      debug!("Installing plugin {} version {}", manifest.name, manifest.version);
      let plugin_dir = create_plugin_dir(&manifest)
        .context("Failed to creating plugins directory")?;
      download_plugin_executable(&manifest, &plugin_dir, &http_client, url, &tag, false).await?;

      Ok(PactPluginManifest {
        plugin_dir: plugin_dir.to_string_lossy().to_string(),
        .. manifest
      })
    } else {
      bail!("GitHub release page does not have a valid tag_name attribute");
    }
  } else {
    bail!("Response from source is not a valid JSON from a GitHub release page")
  }
}

fn create_plugin_dir(manifest: &PactPluginManifest) -> anyhow::Result<PathBuf> {
  let plugins_dir = pact_plugin_dir()?;
  if !plugins_dir.exists() {
    info!(plugins_dir = %plugins_dir.display(), "Creating plugins directory");
    fs::create_dir_all(plugins_dir.clone())?;
  }

  let plugin_dir = plugins_dir.join(format!("{}-{}", manifest.name, manifest.version));
  info!(plugin_dir = %plugin_dir.display(), "Creating plugin directory");
  fs::create_dir(plugin_dir.clone())?;

  info!("Writing plugin manifest file");
  let file_name = plugin_dir.join("pact-plugin.json");
  let mut f = File::create(file_name)?;
  let json = serde_json::to_string(manifest)?;
  f.write_all(json.as_bytes())?;

  Ok(plugin_dir.clone())
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;
  use std::fs::{self, File};
  use std::sync::{Arc, RwLock};
  use async_trait::async_trait;

  use expectest::prelude::*;
  use lazy_static::lazy_static;
  use maplit::hashmap;
  use pact_models::bodies::OptionalBody;
  use pact_models::content_types::ContentType;
  use pact_models::pact::Pact;
  use pact_models::prelude::v4::V4Pact;
  use pact_models::v4::interaction::V4Interaction;
  use pact_models::v4::sync_message::SynchronousMessage;
  use serde_json::Value;
  use tempdir::TempDir;
  use crate::content::InteractionContents;

  use crate::mock_server::MockServerConfig;
  use crate::plugin_manager::prepare_verification_for_interaction_inner;
  use crate::plugin_manager::verify_interaction_inner;
  use crate::plugin_models::{CompareContentRequest, CompareContentResult, PactPlugin, PluginDependency};
  use crate::verification::{InteractionVerificationData, InteractionVerificationResult};

  use super::{
    load_manifest_from_dir,
    PactPluginManifest
  };

  #[test]
  fn load_manifest_from_dir_test() {
    let tmp_dir = TempDir::new("load_manifest_from_dir").unwrap();

    let manifest_1 = PactPluginManifest {
      name: "test-plugin".to_string(),
      version: "0.1.5".to_string(),
      .. PactPluginManifest::default()
    };
    let path_1 = tmp_dir.path().join("1");
    fs::create_dir_all(&path_1).unwrap();
    let file_1 = File::create(path_1.join("pact-plugin.json")).unwrap();
    serde_json::to_writer(file_1, &manifest_1).unwrap();

    let manifest_2 = PactPluginManifest {
      name: "test-plugin".to_string(),
      version: "0.1.20".to_string(),
      .. PactPluginManifest::default()
    };
    let path_2 = tmp_dir.path().join("2");
    fs::create_dir_all(&path_2).unwrap();
    let file_2 = File::create(path_2.join("pact-plugin.json")).unwrap();
    serde_json::to_writer(file_2, &manifest_2).unwrap();

    let manifest_3 = PactPluginManifest {
      name: "test-plugin".to_string(),
      version: "0.1.7".to_string(),
      .. PactPluginManifest::default()
    };
    let path_3 = tmp_dir.path().join("3");
    fs::create_dir_all(&path_3).unwrap();
    let file_3 = File::create(path_3.join("pact-plugin.json")).unwrap();
    serde_json::to_writer(file_3, &manifest_3).unwrap();

    let manifest_4 = PactPluginManifest {
      name: "test-plugin".to_string(),
      version: "0.1.14".to_string(),
      .. PactPluginManifest::default()
    };
    let path_4 = tmp_dir.path().join("4");
    fs::create_dir_all(&path_4).unwrap();
    let file_4 = File::create(path_4.join("pact-plugin.json")).unwrap();
    serde_json::to_writer(file_4, &manifest_4).unwrap();

    let manifest_5 = PactPluginManifest {
      name: "test-plugin".to_string(),
      version: "0.1.12".to_string(),
      .. PactPluginManifest::default()
    };
    let path_5 = tmp_dir.path().join("5");
    fs::create_dir_all(&path_5).unwrap();
    let file_5 = File::create(path_5.join("pact-plugin.json")).unwrap();
    serde_json::to_writer(file_5, &manifest_5).unwrap();

    let dep = PluginDependency {
      name: "test-plugin".to_string(),
      version: None,
      dependency_type: Default::default()
    };

    let result = load_manifest_from_dir(&dep, &tmp_dir.path().to_path_buf()).unwrap();
    expect!(result.version).to(be_equal_to("0.1.20"));
  }

  lazy_static!{
     pub(crate) static ref PREPARE_INTERACTION_FOR_VERIFICATION_ARG: RwLock<Option<V4Pact>> = RwLock::new(None);
     pub(crate) static ref VERIFY_INTERACTION_ARG: RwLock<Option<V4Pact>> = RwLock::new(None);
  }

  #[derive(Default, Debug, Clone)]
  pub(crate) struct MockPlugin {}

  #[async_trait]
  impl PactPlugin for MockPlugin {
    fn manifest(&self) -> PactPluginManifest {
      unimplemented!()
    }

    fn kill(&self) {
      unimplemented!()
    }

    fn update_access(&mut self) {
      unimplemented!()
    }

    fn drop_access(&mut self) -> usize {
      unimplemented!()
    }

    fn boxed(&self) -> Box<dyn PactPlugin + Send + Sync> {
      Box::new(self.clone())
    }

    fn arced(&self) -> Arc<dyn PactPlugin + Send + Sync> {
      Arc::new(self.clone())
    }

    async fn publish_updated_catalogue(&self, _catalogue: &[crate::catalogue_manager::CatalogueEntry]) -> anyhow::Result<()> {
      unimplemented!()
    }

    async fn generate_contents(&self, _request: crate::plugin_models::GenerateContentRequest) -> anyhow::Result<OptionalBody> {
      unimplemented!()
    }

    async fn match_contents(&self, _request: CompareContentRequest) -> anyhow::Result<CompareContentResult> {
      unimplemented!()
    }

    async fn configure_interaction(&self, _content_type: &ContentType, _definition: &HashMap<String, Value>) -> anyhow::Result<(Vec<InteractionContents>, Option<crate::content::PluginConfiguration>)> {
      unimplemented!()
    }

    async fn verify_interaction(&self, pact: &V4Pact, _interaction: &(dyn V4Interaction + Send + Sync), _verification_data: &InteractionVerificationData, _config: &HashMap<String, Value>) -> anyhow::Result<InteractionVerificationResult> {
      let mut w = VERIFY_INTERACTION_ARG.write().unwrap();
      let _ = w.insert(pact.clone());
      Ok(InteractionVerificationResult {
        ok: true,
        details: vec![],
        output: vec![]
      })
    }

    async fn prepare_interaction_for_verification(&self, pact: &V4Pact, _interaction: &(dyn V4Interaction + Send + Sync), _context: &HashMap<String, Value>) -> anyhow::Result<InteractionVerificationData> {
      let mut w = PREPARE_INTERACTION_FOR_VERIFICATION_ARG.write().unwrap();
      let _ = w.insert(pact.clone());
      Ok(InteractionVerificationData {
        request_data: Default::default(),
        metadata: Default::default(),
      })
    }

    async fn start_mock_server(&self, _config: &MockServerConfig, _pact: Box<dyn Pact + Send + Sync>, _test_context: HashMap<String, Value>) -> anyhow::Result<crate::mock_server::MockServerDetails> {
      unimplemented!()
    }


    async fn get_mock_server_results(&self, _mock_server_key: &str) -> anyhow::Result<Vec<crate::mock_server::MockServerResults>> {
      unimplemented!()
    }

    async fn shutdown_mock_server(&self, _mock_server_key: &str) -> anyhow::Result<Vec<crate::mock_server::MockServerResults>> {
      unimplemented!()
    }
  }

  #[test_log::test(tokio::test)]
  async fn prepare_validation_for_interaction_passes_in_pact_with_interaction_keys_set() {
    let manifest = PactPluginManifest {
      name: "test-plugin".to_string(),
      version: "0.0.0".to_string(),
      .. PactPluginManifest::default()
    };
    let mock_plugin = MockPlugin {
      .. MockPlugin::default()
    };

    let interaction = SynchronousMessage {
      .. SynchronousMessage::default()
    };
    let pact = V4Pact {
      interactions: vec![ interaction.boxed_v4() ],
      .. V4Pact::default()
    };
    let context = hashmap!{};

    let result = prepare_verification_for_interaction_inner(
      &mock_plugin,
      &manifest,
      &pact,
      &interaction,
      &context
    ).await;

    expect!(result).to(be_ok());
    let r = PREPARE_INTERACTION_FOR_VERIFICATION_ARG.read().unwrap();
    let pact_in = r.as_ref().unwrap();
    expect!(pact_in.interactions[0].key()).to(be_some());
  }

  #[test_log::test(tokio::test)]
  async fn verify_interaction_passes_in_pact_with_interaction_keys_set() {
    let manifest = PactPluginManifest {
      name: "test-plugin".to_string(),
      version: "0.0.0".to_string(),
      .. PactPluginManifest::default()
    };
    let mock_plugin = MockPlugin {
      .. MockPlugin::default()
    };

    let interaction = SynchronousMessage {
      .. SynchronousMessage::default()
    };
    let pact = V4Pact {
      interactions: vec![ interaction.boxed_v4() ],
      .. V4Pact::default()
    };
    let context = hashmap!{};
    let data = InteractionVerificationData::default();

    let result = verify_interaction_inner(
      &mock_plugin,
      &manifest,
      &data,
      &context,
      &pact,
      &interaction
    ).await;

    expect!(result).to(be_ok());
    let r = VERIFY_INTERACTION_ARG.read().unwrap();
    let pact_in = r.as_ref().unwrap();
    expect!(pact_in.interactions[0].key()).to(be_some());
  }
}
