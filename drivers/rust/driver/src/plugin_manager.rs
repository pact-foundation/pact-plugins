//! Manages interactions with Pact plugins
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::str::from_utf8;
use std::str::FromStr;
use std::sync::Mutex;
use std::thread;

use anyhow::{anyhow, bail, Context};
use bytes::Bytes;
use itertools::Either;
use lazy_static::lazy_static;
use log::max_level;
use maplit::hashmap;
use os_info::Type;
use pact_models::bodies::OptionalBody;
use pact_models::json_utils::json_to_string;
use pact_models::PactSpecification;
use pact_models::prelude::{ContentType, Pact};
use pact_models::prelude::v4::V4Pact;
use pact_models::v4::interaction::V4Interaction;
use reqwest::Client;
use semver::Version;
use serde_json::Value;
use sysinfo::{Pid, PidExt, ProcessExt, Signal, System, SystemExt};
use tokio::process::Command;
use tracing::{debug, error, info, trace, warn};

use crate::catalogue_manager::{all_entries, CatalogueEntry, register_plugin_entries, remove_plugin_entries};
use crate::child_process::ChildPluginProcess;
use crate::content::ContentMismatch;
use crate::download::{download_json_from_github, download_plugin_executable, fetch_json_from_url};
use crate::metrics::send_metrics;
use crate::mock_server::{MockServerConfig, MockServerDetails, MockServerResults};
use crate::plugin_models::{PactPlugin, PactPluginManifest, PactPluginRpc, PluginDependency};
use crate::proto::*;
use crate::repository::{fetch_repository_index, USER_AGENT};
use crate::utils::{optional_string, proto_value_to_json, to_proto_struct, to_proto_value, versions_compatible};
use crate::verification::{InteractionVerificationData, InteractionVerificationResult};

lazy_static! {
  static ref PLUGIN_MANIFEST_REGISTER: Mutex<HashMap<String, PactPluginManifest>> = Mutex::new(HashMap::new());
  static ref PLUGIN_REGISTER: Mutex<HashMap<String, PactPlugin>> = Mutex::new(HashMap::new());
}

/// Load the plugin defined by the dependency information. Will first look in the global
/// plugin registry.
pub async fn load_plugin(plugin: &PluginDependency) -> anyhow::Result<PactPlugin> {
  let thread_id = thread::current().id();
  debug!("Loading plugin {:?}", plugin);
  trace!("Rust plugin driver version {}", option_env!("CARGO_PKG_VERSION").unwrap_or_default());
  trace!("load_plugin {:?}: Waiting on PLUGIN_REGISTER lock", thread_id);
  let mut inner = PLUGIN_REGISTER.lock().unwrap();
  trace!("load_plugin {:?}: Got PLUGIN_REGISTER lock", thread_id);
  let result = match lookup_plugin_inner(plugin, &mut inner) {
    Some(plugin) => {
      debug!("Found running plugin {:?}", plugin);
      plugin.update_access();
      Ok(plugin.clone())
    },
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
    }
  };
  trace!("load_plugin {:?}: Releasing PLUGIN_REGISTER lock", thread_id);
  result
}

fn lookup_plugin_inner<'a>(
  plugin: &PluginDependency,
  plugin_register: &'a mut HashMap<String, PactPlugin>
) -> Option<&'a mut PactPlugin> {
  if let Some(version) = &plugin.version {
    plugin_register.get_mut(format!("{}/{}", plugin.name, version).as_str())
  } else {
    plugin_register.iter_mut()
      .filter(|(_, value)| value.manifest.name == plugin.name)
      .max_by(|(_, v1), (_, v2)| v1.manifest.version.cmp(&v2.manifest.version))
      .map(|(_, plugin)| plugin)
  }
}

/// Look up the plugin in the global plugin register
pub fn lookup_plugin(plugin: &PluginDependency) -> Option<PactPlugin> {
  let thread_id = thread::current().id();
  trace!("lookup_plugin {:?}: Waiting on PLUGIN_REGISTER lock", thread_id);
  let mut inner = PLUGIN_REGISTER.lock().unwrap();
  trace!("lookup_plugin {:?}: Got PLUGIN_REGISTER lock", thread_id);
  let entry = lookup_plugin_inner(plugin, &mut inner);
  trace!("lookup_plugin {:?}: Releasing PLUGIN_REGISTER lock", thread_id);
  entry.cloned()
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
    home::home_dir().map(|dir| dir.join(".pact/plugins"))
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

async fn initialise_plugin(
  manifest: &PactPluginManifest,
  plugin_register: &mut HashMap<String, PactPlugin>
) -> anyhow::Result<PactPlugin> {
  match manifest.executable_type.as_str() {
    "exec" => {
      let mut plugin = start_plugin_process(manifest).await?;
      debug!("Plugin process started OK (port = {}), sending init message", plugin.port());

      init_handshake(manifest, &mut plugin).await.map_err(|err| {
        plugin.kill();
        anyhow!("Failed to send init request to the plugin - {}", err)
      })?;

      let key = format!("{}/{}", manifest.name, manifest.version);
      plugin_register.insert(key, plugin.clone());

      Ok(plugin)
    }
    _ => Err(anyhow!("Plugin executable type of {} is not supported", manifest.executable_type))
  }
}

/// Internal function: public for testing
pub async fn init_handshake(manifest: &PactPluginManifest, plugin: &mut (dyn PactPluginRpc + Send + Sync)) -> anyhow::Result<()> {
  let request = InitPluginRequest {
    implementation: "plugin-driver-rust".to_string(),
    version: option_env!("CARGO_PKG_VERSION").unwrap_or("0").to_string()
  };
  let response = plugin.init_plugin(request).await?;
  debug!("Got init response {:?} from plugin {}", response, manifest.name);
  register_plugin_entries(manifest, &response.catalogue);
  tokio::task::spawn(publish_updated_catalogue());
  Ok(())
}

async fn start_plugin_process(manifest: &PactPluginManifest) -> anyhow::Result<PactPlugin> {
  debug!("Starting plugin with manifest {:?}", manifest);

  let os_info = os_info::get();
  debug!("Detected OS: {}", os_info);
  let mut path = if let Some(entry_point) = manifest.entry_points.get(&os_info.to_string()) {
    PathBuf::from(entry_point)
  } else if os_info.os_type() == Type::Windows && manifest.entry_points.contains_key("windows") {
    PathBuf::from(manifest.entry_points.get("windows").unwrap())
  } else {
    PathBuf::from(&manifest.entry_point)
  };

  if !path.is_absolute() || !path.exists() {
    path = PathBuf::from(manifest.plugin_dir.clone()).join(path);
  }
  debug!("Starting plugin using {:?}", &path);

  let log_level = max_level();
  let mut child_command = Command::new(path.clone());
  let mut child_command = child_command
    .env("LOG_LEVEL", log_level.to_string())
    .env("RUST_LOG", log_level.to_string())
    .current_dir(manifest.plugin_dir.clone());

  if let Some(args) = &manifest.args {
    child_command = child_command.args(args);
  }

  let child = child_command
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| anyhow!("Was not able to start plugin process for '{}' - {}",
      path.to_string_lossy(), err))?;
  let child_pid = child.id().unwrap_or_default();
  debug!("Plugin {} started with PID {}", manifest.name, child_pid);

  match ChildPluginProcess::new(child, manifest).await {
    Ok(child) => Ok(PactPlugin::new(manifest, child)),
    Err(err) => {
      let mut s = System::new();
      s.refresh_processes();
      if let Some(process) = s.process(Pid::from_u32(child_pid)) {
        process.kill_with(Signal::Term);
      } else {
        warn!("Child process with PID {} was not found", child_pid);
      }
      Err(err)
    }
  }
}

/// Shut down all plugin processes
pub fn shutdown_plugins() {
  let thread_id = thread::current().id();
  debug!("Shutting down all plugins");
  trace!("shutdown_plugins {:?}: Waiting on PLUGIN_REGISTER lock", thread_id);
  let mut guard = PLUGIN_REGISTER.lock().unwrap();
  trace!("shutdown_plugins {:?}: Got PLUGIN_REGISTER lock", thread_id);
  for plugin in guard.values() {
    debug!("Shutting down plugin {:?}", plugin);
    plugin.kill();
    remove_plugin_entries(&plugin.manifest.name);
  }
  guard.clear();
  trace!("shutdown_plugins {:?}: Releasing PLUGIN_REGISTER lock", thread_id);
}

/// Shutdown the given plugin
pub fn shutdown_plugin(plugin: &mut PactPlugin) {
  debug!("Shutting down plugin {}:{}", plugin.manifest.name, plugin.manifest.version);
  plugin.kill();
  remove_plugin_entries(&plugin.manifest.name);
}

/// Publish the current catalogue to all plugins
pub async fn publish_updated_catalogue() {
  let thread_id = thread::current().id();

  let request = Catalogue {
    catalogue: all_entries().iter()
      .map(|entry| crate::proto::CatalogueEntry {
        r#type: entry.entry_type.to_proto_type() as i32,
        key: entry.key.clone(),
        values: entry.values.clone()
      }).collect()
  };

  let plugins = {
    trace!("publish_updated_catalogue {:?}: Waiting on PLUGIN_REGISTER lock", thread_id);
    let inner = PLUGIN_REGISTER.lock().unwrap();
    trace!("publish_updated_catalogue {:?}: Got PLUGIN_REGISTER lock", thread_id);
    let plugins = inner.values().cloned().collect::<Vec<_>>();
    trace!("publish_updated_catalogue {:?}: Releasing PLUGIN_REGISTER lock", thread_id);
    plugins
  };

  for plugin in plugins {
    if let Err(err) = plugin.update_catalogue(request.clone()).await {
      warn!("Failed to send updated catalogue to plugin '{}' - {}", plugin.manifest.name, err);
    }
  }
}

/// Increment access to the plugin.
#[tracing::instrument]
pub fn increment_plugin_access(plugin: &PluginDependency) {
  let thread_id = thread::current().id();

  trace!("increment_plugin_access {:?}: Waiting on PLUGIN_REGISTER lock", thread_id);
  let mut inner = PLUGIN_REGISTER.lock().unwrap();
  trace!("increment_plugin_access {:?}: Got PLUGIN_REGISTER lock", thread_id);

  if let Some(plugin) = lookup_plugin_inner(plugin, &mut inner) {
    plugin.update_access();
  }

  trace!("increment_plugin_access {:?}: Releasing PLUGIN_REGISTER lock", thread_id);
}

/// Decrement access to the plugin. If the current access count is zero, shut down the plugin
#[tracing::instrument]
pub fn drop_plugin_access(plugin: &PluginDependency) {
  let thread_id = thread::current().id();

  trace!("drop_plugin_access {:?}: Waiting on PLUGIN_REGISTER lock", thread_id);
  let mut inner = PLUGIN_REGISTER.lock().unwrap();
  trace!("drop_plugin_access {:?}: Got PLUGIN_REGISTER lock", thread_id);

  if let Some(plugin) = lookup_plugin_inner(plugin, &mut inner) {
    let key = format!("{}/{}", plugin.manifest.name, plugin.manifest.version);
    if plugin.drop_access() == 0 {
      shutdown_plugin(plugin);
      inner.remove(key.as_str());
    }
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
  let request = StartMockServerRequest {
    host_interface: config.host_interface.unwrap_or_default(),
    port: config.port,
    tls: config.tls,
    pact: pact.to_json(PactSpecification::V4)?.to_string(),
    test_context: Some(to_proto_struct(&test_context))
  };
  let response = plugin.start_mock_server(request).await?;
  debug!("Got response ${response:?}");

  let mock_server_response = response.response
    .ok_or_else(|| anyhow!("Did not get a valid response from the start mock server call"))?;
  match mock_server_response {
    start_mock_server_response::Response::Error(err) => Err(anyhow!("Mock server failed to start: {}", err)),
    start_mock_server_response::Response::Details(details) => Ok(MockServerDetails {
      key: details.key.clone(),
      base_url: details.address.clone(),
      port: details.port,
      plugin
    })
  }
}

/// Shutdowns a running mock server. Will return any errors from the mock server.
pub async fn shutdown_mock_server(mock_server: &MockServerDetails) -> anyhow::Result<Vec<MockServerResults>> {
  let request = ShutdownMockServerRequest {
    server_key: mock_server.key.to_string()
  };

  debug!(
    plugin_name = mock_server.plugin.manifest.name.as_str(),
    plugin_version = mock_server.plugin.manifest.version.as_str(),
    server_key = mock_server.key.as_str(),
    "Sending shutdownMockServer request to plugin"
  );
  let response = mock_server.plugin.shutdown_mock_server(request).await?;
  debug!("Got response: {response:?}");

  if response.ok {
    Ok(vec![])
  } else {
    Ok(response.results.iter().map(|result| {
      MockServerResults {
        path: result.path.clone(),
        error: result.error.clone(),
        mismatches: result.mismatches.iter().map(|mismatch| {
          ContentMismatch {
            expected: mismatch.expected.as_ref()
              .map(|e| from_utf8(&e).unwrap_or_default().to_string())
              .unwrap_or_default(),
            actual: mismatch.actual.as_ref()
              .map(|a| from_utf8(&a).unwrap_or_default().to_string())
              .unwrap_or_default(),
            mismatch: mismatch.mismatch.clone(),
            path: mismatch.path.clone(),
            diff: optional_string(&mismatch.diff),
            mismatch_type: optional_string(&mismatch.mismatch_type)
          }
        }).collect()
      }
    }).collect())
  }
}

/// Gets the results from a running mock server.
pub async fn get_mock_server_results(mock_server: &MockServerDetails) -> anyhow::Result<Vec<MockServerResults>> {
  let request = MockServerRequest {
    server_key: mock_server.key.to_string()
  };

  debug!(
    plugin_name = mock_server.plugin.manifest.name.as_str(),
    plugin_version = mock_server.plugin.manifest.version.as_str(),
    server_key = mock_server.key.as_str(),
    "Sending getMockServerResults request to plugin"
  );
  let response = mock_server.plugin.get_mock_server_results(request).await?;
  debug!("Got response: {response:?}");

  if response.ok {
    Ok(vec![])
  } else {
    Ok(response.results.iter().map(|result| {
      MockServerResults {
        path: result.path.clone(),
        error: result.error.clone(),
        mismatches: result.mismatches.iter().map(|mismatch| {
          ContentMismatch {
            expected: mismatch.expected.as_ref()
              .map(|e| from_utf8(&e).unwrap_or_default().to_string())
              .unwrap_or_default(),
            actual: mismatch.actual.as_ref()
              .map(|a| from_utf8(&a).unwrap_or_default().to_string())
              .unwrap_or_default(),
            mismatch: mismatch.mismatch.clone(),
            path: mismatch.path.clone(),
            diff: optional_string(&mismatch.diff),
            mismatch_type: optional_string(&mismatch.mismatch_type)
          }
        }).collect()
      }
    }).collect())
  }
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

  let request = VerificationPreparationRequest {
    pact: pact.to_json(PactSpecification::V4)?.to_string(),
    interaction_key: interaction.unique_key(),
    config: Some(to_proto_struct(context))
  };

  debug!(plugin_name = manifest.name.as_str(), plugin_version = manifest.version.as_str(),
    "Sending prepareValidationForInteraction request to plugin");
  let response = plugin.prepare_interaction_for_verification(request).await?;
  debug!("Got response: {response:?}");

  let validation_response = response.response
    .ok_or_else(|| anyhow!("Did not get a valid response from the prepare interaction for verification call"))?;
  match &validation_response {
    verification_preparation_response::Response::Error(err) => Err(anyhow!("Failed to prepate the request: {}", err)),
    verification_preparation_response::Response::InteractionData(data) => {
      let content_type = data.body.as_ref().and_then(|body| ContentType::parse(body.content_type.as_str()).ok());
      Ok(InteractionVerificationData {
        request_data: data.body.as_ref()
          .and_then(|body| body.content.as_ref())
          .map(|body| OptionalBody::Present(Bytes::from(body.clone()), content_type, None)).unwrap_or_default(),
        metadata: data.metadata.iter().map(|(k, v)| {
          let value = match &v.value {
            Some(v) => match &v {
              metadata_value::Value::NonBinaryValue(v) => Either::Left(proto_value_to_json(v)),
              metadata_value::Value::BinaryValue(b) => Either::Right(Bytes::from(b.clone()))
            }
            None => Either::Left(Value::Null)
          };
          (k.clone(), value)
        }).collect()
      })
    }
  }
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

  let request = VerifyInteractionRequest {
    pact: pact.to_json(PactSpecification::V4)?.to_string(),
    interaction_key: interaction.unique_key(),
    config: Some(to_proto_struct(config)),
    interaction_data: Some(InteractionData {
      body: Some((&verification_data.request_data).into()),
      metadata: verification_data.metadata.iter().map(|(k, v)| {
        (k.clone(), MetadataValue { value: Some(match v {
          Either::Left(value) => metadata_value::Value::NonBinaryValue(to_proto_value(value)),
          Either::Right(b) => metadata_value::Value::BinaryValue(b.to_vec())
        }) })
      }).collect()
    })
  };

  debug!(plugin_name = manifest.name.as_str(), plugin_version = manifest.version.as_str(),
    "Sending verifyInteraction request to plugin");
  let response = plugin.verify_interaction(request).await?;
  debug!("Got response: {response:?}");

  let validation_response = response.response
    .ok_or_else(|| anyhow!("Did not get a valid response from the verification call"))?;
  match &validation_response {
    verify_interaction_response::Response::Error(err) => Err(anyhow!("Failed to verify the request: {}", err)),
    verify_interaction_response::Response::Result(data) => Ok(data.into())
  }
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
  use std::fs::{self, File};

  use expectest::prelude::*;
  use tempdir::TempDir;

  use crate::plugin_models::PluginDependency;

  use super::{load_manifest_from_dir, PactPluginManifest};

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
}
