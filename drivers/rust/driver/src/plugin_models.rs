//! Models for representing plugins

use std::collections::HashMap;
use std::sync::Arc;
use anyhow::anyhow;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tonic::metadata::MetadataValue;
use tonic::transport::Channel;
use tracing::{debug, trace};

use crate::child_process::ChildPluginProcess;
use crate::proto::*;
use crate::proto::pact_plugin_client::PactPluginClient;

/// Type of plugin dependencies
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub enum PluginDependencyType {
  /// Required operating system package
  OSPackage,
  /// Dependency on another plugin
  Plugin,
  /// Dependency on a shared library
  Library,
  /// Dependency on an executable
  Executable
}

impl Default for PluginDependencyType {
  fn default() -> Self {
    PluginDependencyType::Plugin
  }
}

/// Plugin dependency
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PluginDependency {
  /// Dependency name
  pub name: String,
  /// Dependency version (semver format)
  pub version: Option<String>,
  /// Type of dependency
  #[serde(default)]
  pub dependency_type: PluginDependencyType
}

/// Manifest of a plugin
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PactPluginManifest {
  /// Directory were the plugin was loaded from
  #[serde(skip)]
  pub plugin_dir: String,

  /// Interface version supported by the plugin
  pub plugin_interface_version: u8,

  /// Plugin name
  pub name: String,

  /// Plugin version in semver format
  pub version: String,

  /// Type if executable of the plugin
  pub executable_type: String,

  /// Minimum required version for the executable type
  pub minimum_required_version: Option<String>,

  /// How to invoke the plugin
  pub entry_point: String,

  /// Additional entry points for other operating systems (i.e. requiring a .bat file for Windows)
  #[serde(default)]
  pub entry_points: HashMap<String, String>,

  /// Dependencies required to invoke the plugin
  pub dependencies: Option<Vec<PluginDependency>>,

  /// Plugin specific config
  #[serde(default)]
  pub plugin_config: HashMap<String, Value>
}

impl PactPluginManifest {
  pub fn as_dependency(&self) -> PluginDependency {
    PluginDependency {
      name: self.name.clone(),
      version: Some(self.version.clone()),
      dependency_type: PluginDependencyType::Plugin
    }
  }
}

impl Default for PactPluginManifest {
  fn default() -> Self {
    PactPluginManifest {
      plugin_dir: "".to_string(),
      plugin_interface_version: 1,
      name: "".to_string(),
      version: "".to_string(),
      executable_type: "".to_string(),
      minimum_required_version: None,
      entry_point: "".to_string(),
      entry_points: Default::default(),
      dependencies: None,
      plugin_config: Default::default()
    }
  }
}

/// Trait with remote-calling methods for a running plugin
#[async_trait]
pub trait PactPluginRpc {
  /// Send an init request to the plugin process
  async fn init_plugin(&self, request: InitPluginRequest) -> anyhow::Result<InitPluginResponse>;

  /// Send a compare contents request to the plugin process
  async fn compare_contents(&self, request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse>;

  /// Send a configure contents request to the plugin process
  async fn configure_interaction(&self, request: ConfigureInteractionRequest) -> anyhow::Result<ConfigureInteractionResponse>;

  /// Send a generate content request to the plugin
  async fn generate_content(&self, request: GenerateContentRequest) -> anyhow::Result<GenerateContentResponse>;

  /// Start a mock server
  async fn start_mock_server(&self, request: StartMockServerRequest) -> anyhow::Result<StartMockServerResponse>;

  /// Shutdown a running mock server
  async fn shutdown_mock_server(&self, request: ShutdownMockServerRequest) -> anyhow::Result<ShutdownMockServerResponse>;

  /// Get the matching results from a running mock server
  async fn get_mock_server_results(&self, request: MockServerRequest) -> anyhow::Result<MockServerResults>;

  /// Prepare an interaction for verification. This should return any data required to construct any request
  /// so that it can be amended before the verification is run.
  async fn prepare_interaction_for_verification(&self, request: VerificationPreparationRequest) -> anyhow::Result<VerificationPreparationResponse>;

  /// Execute the verification for the interaction.
  async fn verify_interaction(&self, request: VerifyInteractionRequest) -> anyhow::Result<VerifyInteractionResponse>;
}

/// Running plugin details
#[derive(Debug, Clone)]
pub struct PactPlugin {
  /// Manifest for this plugin
  pub manifest: PactPluginManifest,

  /// Running child process
  pub child: Arc<ChildPluginProcess>,

  /// Count of access to the plugin. If this is ever zero, the plugin process will be shutdown
  access_count: usize
}

#[async_trait]
impl PactPluginRpc for PactPlugin {
  /// Send an init request to the plugin process
  async fn init_plugin(&self, request: InitPluginRequest) -> anyhow::Result<InitPluginResponse> {
    let channel = self.connect_channel().await?;
    let auth_str = self.child.plugin_info.server_key.as_str();
    let token = MetadataValue::from_str(auth_str)?;
    let mut client = PactPluginClient::with_interceptor(channel, move |mut req: tonic::Request<_>| {
      req.metadata_mut().insert("authorization", token.clone());
      Ok(req)
    });
    let response = client.init_plugin(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  /// Send a compare contents request to the plugin process
  async fn compare_contents(&self, request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse> {
    let channel = self.connect_channel().await?;
    let auth_str = self.child.plugin_info.server_key.as_str();
    let token = MetadataValue::from_str(auth_str)?;
    let mut client = PactPluginClient::with_interceptor(channel, move |mut req: tonic::Request<_>| {
      req.metadata_mut().insert("authorization", token.clone());
      Ok(req)
    });
    let response = client.compare_contents(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  /// Send a configure contents request to the plugin process
  async fn configure_interaction(&self, request: ConfigureInteractionRequest) -> anyhow::Result<ConfigureInteractionResponse> {
    let channel = self.connect_channel().await?;
    let auth_str = self.child.plugin_info.server_key.as_str();
    let token = MetadataValue::from_str(auth_str)?;
    let mut client = PactPluginClient::with_interceptor(channel, move |mut req: tonic::Request<_>| {
      req.metadata_mut().insert("authorization", token.clone());
      Ok(req)
    });
    let response = client.configure_interaction(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  /// Send a generate content request to the plugin
  async fn generate_content(&self, request: GenerateContentRequest) -> anyhow::Result<GenerateContentResponse> {
    let channel = self.connect_channel().await?;
    let auth_str = self.child.plugin_info.server_key.as_str();
    let token = MetadataValue::from_str(auth_str)?;
    let mut client = PactPluginClient::with_interceptor(channel, move |mut req: tonic::Request<_>| {
      req.metadata_mut().insert("authorization", token.clone());
      Ok(req)
    });
    let response = client.generate_content(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn start_mock_server(&self, request: StartMockServerRequest) -> anyhow::Result<StartMockServerResponse> {
    let channel = self.connect_channel().await?;
    let auth_str = self.child.plugin_info.server_key.as_str();
    let token = MetadataValue::from_str(auth_str)?;
    let mut client = PactPluginClient::with_interceptor(channel, move |mut req: tonic::Request<_>| {
      req.metadata_mut().insert("authorization", token.clone());
      Ok(req)
    });
    let response = client.start_mock_server(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn shutdown_mock_server(&self, request: ShutdownMockServerRequest) -> anyhow::Result<ShutdownMockServerResponse> {
    let channel = self.connect_channel().await?;
    let auth_str = self.child.plugin_info.server_key.as_str();
    let token = MetadataValue::from_str(auth_str)?;
    let mut client = PactPluginClient::with_interceptor(channel, move |mut req: tonic::Request<_>| {
      req.metadata_mut().insert("authorization", token.clone());
      Ok(req)
    });
    let response = client.shutdown_mock_server(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn get_mock_server_results(&self, request: MockServerRequest) -> anyhow::Result<MockServerResults> {
    let channel = self.connect_channel().await?;
    let auth_str = self.child.plugin_info.server_key.as_str();
    let token = MetadataValue::from_str(auth_str)?;
    let mut client = PactPluginClient::with_interceptor(channel, move |mut req: tonic::Request<_>| {
      req.metadata_mut().insert("authorization", token.clone());
      Ok(req)
    });
    let response = client.get_mock_server_results(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn prepare_interaction_for_verification(&self, request: VerificationPreparationRequest) -> anyhow::Result<VerificationPreparationResponse> {
    let channel = self.connect_channel().await?;
    let auth_str = self.child.plugin_info.server_key.as_str();
    let token = MetadataValue::from_str(auth_str)?;
    let mut client = PactPluginClient::with_interceptor(channel, move |mut req: tonic::Request<_>| {
      req.metadata_mut().insert("authorization", token.clone());
      Ok(req)
    });
    let response = client.prepare_interaction_for_verification(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn verify_interaction(&self, request: VerifyInteractionRequest) -> anyhow::Result<VerifyInteractionResponse> {
    let channel = self.connect_channel().await?;
    let auth_str = self.child.plugin_info.server_key.as_str();
    let token = MetadataValue::from_str(auth_str)?;
    let mut client = PactPluginClient::with_interceptor(channel, move |mut req: tonic::Request<_>| {
      req.metadata_mut().insert("authorization", token.clone());
      Ok(req)
    });
    let response = client.verify_interaction(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }
}

impl PactPlugin {
  /// Create a new Plugin
  pub fn new(manifest: &PactPluginManifest, child: ChildPluginProcess) -> Self {
    PactPlugin { manifest: manifest.clone(), child: Arc::new(child), access_count: 1 }
  }

  /// Port the plugin is running on
  pub fn port(&self) -> u16 {
    self.child.port()
  }

  /// Kill the running plugin process
  pub fn kill(&self) {
    self.child.kill();
  }

  /// Update the access of the plugin
  pub fn update_access(&mut self) {
    self.access_count += 1;
    trace!("update_access: Plugin {}/{} access is now {}", self.manifest.name, self.manifest.version,
      self.access_count);
  }

  /// Decrement and return the access count for the plugin
  pub fn drop_access(&mut self) -> usize {
    if self.access_count > 0 {
      self.access_count -= 1;
    }
    trace!("drop_access: Plugin {}/{} access is now {}", self.manifest.name, self.manifest.version,
      self.access_count);
    self.access_count
  }

  async fn connect_channel(&self) -> anyhow::Result<Channel> {
    let port = self.child.port();
    match Channel::from_shared(format!("http://[::1]:{}", port))?.connect().await {
      Ok(channel) => Ok(channel),
      Err(err) => {
        debug!("IP6 connection failed, will try IP4 address - {err}");
        Channel::from_shared(format!("http://127.0.0.1:{}", port))?.connect().await
          .map_err(|err| anyhow!(err))
      }
    }
  }
}

/// Plugin configuration to add to the matching context for an interaction
#[derive(Clone, Debug, PartialEq)]
pub struct PluginInteractionConfig {
  /// Global plugin config (Pact level)
  pub pact_configuration: HashMap<String, Value>,
  /// Interaction plugin config
  pub interaction_configuration: HashMap<String, Value>
}
