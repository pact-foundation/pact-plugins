//! Models for representing plugins

use serde::{Deserialize, Serialize};

use crate::child_process::ChildPluginProcess;
use crate::proto::*;
use crate::proto::pact_plugin_client::PactPluginClient;

/// Type of plugin dependencies
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
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
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
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
  /// Dependencies required to invoke the plugin
  pub dependencies: Option<Vec<PluginDependency>>
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

/// Running plugin details
#[derive(Debug, Clone)]
pub struct PactPlugin {
  /// Manifest for this plugin
  pub manifest: PactPluginManifest,
  /// Running child process
  pub child: ChildPluginProcess
}

impl PactPlugin {
  /// Create a new Plugin
  pub fn new(manifest: &PactPluginManifest, child: ChildPluginProcess) -> Self {
    PactPlugin { manifest: manifest.clone(), child }
  }

  /// Port the plugin is running on
  pub fn port(&self) -> u16 {
    self.child.port()
  }

  /// Kill the running plugin process
  pub fn kill(&self) {
    self.child.kill();
  }

  /// Send an init request to the plugin process
  pub async fn init_plugin(&self, request: InitPluginRequest) -> anyhow::Result<InitPluginResponse> {
    let mut client = PactPluginClient::connect(format!("http://127.0.0.1:{}", self.child.port())).await?;
    let response = client.init_plugin(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  /// Send a compare contents request to the plugin process
  pub async fn compare_contents(&self, request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse> {
    let mut client = PactPluginClient::connect(format!("http://127.0.0.1:{}", self.child.port())).await?;
    let response = client.compare_contents(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  /// Send a configure contents request to the plugin process
  pub async fn configure_contents(&self, request: ConfigureContentsRequest) -> anyhow::Result<ConfigureContentsResponse> {
    let mut client = PactPluginClient::connect(format!("http://127.0.0.1:{}", self.child.port())).await?;
    let response = client.configure_contents(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  /// Send a generate content request to the plugin
  pub async fn generate_content(&self, request: GenerateContentRequest) -> anyhow::Result<GenerateContentResponse> {
    let mut client = PactPluginClient::connect(format!("http://127.0.0.1:{}", self.child.port())).await?;
    let response = client.generate_content(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }
}
