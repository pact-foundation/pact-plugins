//! Models for representing plugins

use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};

use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::proto::*;
use crate::proto_v2;

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
  Executable,
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
  pub dependency_type: PluginDependencyType,
}

impl Display for PluginDependency {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    if let Some(version) = &self.version {
      write!(f, "{}:{}", self.name, version)
    } else {
      write!(f, "{}:*", self.name)
    }
  }
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

  /// Parameters to pass into the command line
  pub args: Option<Vec<String>>,

  /// Dependencies required to invoke the plugin
  pub dependencies: Option<Vec<PluginDependency>>,

  /// Plugin specific config
  #[serde(default)]
  pub plugin_config: HashMap<String, Value>,
}

impl PactPluginManifest {
  pub fn as_dependency(&self) -> PluginDependency {
    PluginDependency {
      name: self.name.clone(),
      version: Some(self.version.clone()),
      dependency_type: PluginDependencyType::Plugin,
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
      args: None,
      dependencies: None,
      plugin_config: Default::default(),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginInterfaceVersion {
  V1,
  V2,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginInitRequest {
  pub implementation: String,
  pub version: String,
  pub host_capabilities: Vec<String>,
  pub plugin_instance_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PluginInitResponse {
  pub catalogue: Vec<CatalogueEntry>,
  pub plugin_capabilities: Vec<String>,
}

impl TryFrom<u8> for PluginInterfaceVersion {
  type Error = anyhow::Error;

  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      1 => Ok(PluginInterfaceVersion::V1),
      2 => Ok(PluginInterfaceVersion::V2),
      _ => Err(anyhow!("Unsupported plugin interface version {}", value)),
    }
  }
}

/// Trait for initialising a plugin via the init handshake
#[async_trait]
pub trait PactPluginRpc {
  /// Send an init request to the plugin process
  async fn init_plugin(
    &mut self,
    request: PluginInitRequest,
  ) -> anyhow::Result<PluginInitResponse>;
}

/// Trait representing an active plugin instance.
///
/// The default implementations for the V2-only methods return an error, matching the
/// behaviour of a V1-only plugin.
#[async_trait]
pub trait PactPlugin: Debug + Send + Sync {
  /// Manifest for this plugin
  fn manifest(&self) -> &PactPluginManifest;

  /// Kill the running plugin process
  fn kill(&self);

  /// Increment the access count (interior mutability)
  fn update_access(&self);

  /// Decrement and return the access count (interior mutability)
  fn drop_access(&self) -> usize;

  /// Instance ID assigned at process start
  fn instance_id(&self) -> &str;

  /// Check whether the plugin declared a specific capability
  fn has_capability(&self, capability: &str) -> bool;

  /// Send a compare contents request to the plugin process
  async fn compare_contents(
    &self,
    request: CompareContentsRequest,
  ) -> anyhow::Result<CompareContentsResponse>;

  /// Send a configure interaction request to the plugin process
  async fn configure_interaction(
    &self,
    request: ConfigureInteractionRequest,
  ) -> anyhow::Result<ConfigureInteractionResponse>;

  /// Send a generate content request to the plugin
  async fn generate_content(
    &self,
    request: GenerateContentRequest,
  ) -> anyhow::Result<GenerateContentResponse>;

  /// Start a mock server (V1 — pact JSON wire format)
  async fn start_mock_server(
    &self,
    request: StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse>;

  /// Start a mock server using V2 structured interaction data (no pact JSON)
  async fn start_mock_server_v2(
    &self,
    request: proto_v2::StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse> {
    let _ = request;
    Err(anyhow!("V2 interface not supported by this plugin"))
  }

  /// Shutdown a running mock server
  async fn shutdown_mock_server(
    &self,
    request: ShutdownMockServerRequest,
  ) -> anyhow::Result<ShutdownMockServerResponse>;

  /// Get the matching results from a running mock server
  async fn get_mock_server_results(
    &self,
    request: MockServerRequest,
  ) -> anyhow::Result<MockServerResults>;

  /// Prepare an interaction for verification
  async fn prepare_interaction_for_verification(
    &self,
    request: VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse>;

  /// Prepare an interaction for verification using V2 structured data
  async fn prepare_interaction_for_verification_v2(
    &self,
    request: proto_v2::VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse> {
    let _ = request;
    Err(anyhow!("V2 interface not supported by this plugin"))
  }

  /// Execute the verification for the interaction
  async fn verify_interaction(
    &self,
    request: VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse>;

  /// Execute the verification for the interaction using V2 structured data
  async fn verify_interaction_v2(
    &self,
    request: proto_v2::VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse> {
    let _ = request;
    Err(anyhow!("V2 interface not supported by this plugin"))
  }

  /// Updates the catalogue (sent when the core catalogue has been updated)
  async fn update_catalogue(&self, request: Catalogue) -> anyhow::Result<()>;
}

/// Plugin configuration to add to the matching context for an interaction
#[derive(Clone, Debug, PartialEq)]
pub struct PluginInteractionConfig {
  /// Global plugin config (Pact level)
  pub pact_configuration: HashMap<String, Value>,
  /// Interaction plugin config
  pub interaction_configuration: HashMap<String, Value>,
}

#[cfg(test)]
pub(crate) mod tests {
  use std::sync::RwLock;

  use async_trait::async_trait;

  use crate::plugin_models::{
    PactPlugin, PactPluginManifest, PluginInitResponse,
  };
  use crate::proto::verification_preparation_response::Response;
  use crate::proto::*;

  /// Test double that records the last prepare/verify requests it received.
  pub(crate) struct MockPlugin {
    pub manifest: PactPluginManifest,
    pub prepare_request: RwLock<VerificationPreparationRequest>,
    pub verify_request: RwLock<VerifyInteractionRequest>,
  }

  impl std::fmt::Debug for MockPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("MockPlugin")
        .field("manifest", &self.manifest)
        .finish()
    }
  }

  impl Default for MockPlugin {
    fn default() -> Self {
      MockPlugin {
        manifest: PactPluginManifest::default(),
        prepare_request: RwLock::new(VerificationPreparationRequest::default()),
        verify_request: RwLock::new(VerifyInteractionRequest::default()),
      }
    }
  }

  #[async_trait]
  impl PactPlugin for MockPlugin {
    fn manifest(&self) -> &PactPluginManifest {
      &self.manifest
    }

    fn kill(&self) {}

    fn update_access(&self) {}

    fn drop_access(&self) -> usize {
      1
    }

    fn instance_id(&self) -> &str {
      "test-instance"
    }

    fn has_capability(&self, _capability: &str) -> bool {
      false
    }

    async fn compare_contents(
      &self,
      _request: CompareContentsRequest,
    ) -> anyhow::Result<CompareContentsResponse> {
      unimplemented!()
    }

    async fn configure_interaction(
      &self,
      _request: ConfigureInteractionRequest,
    ) -> anyhow::Result<ConfigureInteractionResponse> {
      unimplemented!()
    }

    async fn generate_content(
      &self,
      _request: GenerateContentRequest,
    ) -> anyhow::Result<GenerateContentResponse> {
      unimplemented!()
    }

    async fn start_mock_server(
      &self,
      _request: StartMockServerRequest,
    ) -> anyhow::Result<StartMockServerResponse> {
      unimplemented!()
    }

    async fn shutdown_mock_server(
      &self,
      _request: ShutdownMockServerRequest,
    ) -> anyhow::Result<ShutdownMockServerResponse> {
      unimplemented!()
    }

    async fn get_mock_server_results(
      &self,
      _request: MockServerRequest,
    ) -> anyhow::Result<MockServerResults> {
      unimplemented!()
    }

    async fn prepare_interaction_for_verification(
      &self,
      request: VerificationPreparationRequest,
    ) -> anyhow::Result<VerificationPreparationResponse> {
      let mut w = self.prepare_request.write().unwrap();
      *w = request;
      let data = InteractionData {
        body: None,
        metadata: Default::default(),
      };
      Ok(VerificationPreparationResponse {
        response: Some(Response::InteractionData(data)),
      })
    }

    async fn verify_interaction(
      &self,
      request: VerifyInteractionRequest,
    ) -> anyhow::Result<VerifyInteractionResponse> {
      let mut w = self.verify_request.write().unwrap();
      *w = request;
      let result = VerificationResult {
        success: false,
        response_data: None,
        mismatches: vec![],
        output: vec![],
      };
      Ok(VerifyInteractionResponse {
        response: Some(verify_interaction_response::Response::Result(result)),
      })
    }

    async fn update_catalogue(&self, _request: Catalogue) -> anyhow::Result<()> {
      unimplemented!()
    }
  }

  /// Minimal PactPluginRpc implementation that returns a fixed error from init_plugin.
  pub(crate) struct FailingInitPlugin {
    pub error: String,
  }

  #[async_trait]
  impl crate::plugin_models::PactPluginRpc for FailingInitPlugin {
    async fn init_plugin(
      &mut self,
      _request: crate::plugin_models::PluginInitRequest,
    ) -> anyhow::Result<PluginInitResponse> {
      Err(anyhow::anyhow!("{}", self.error))
    }
  }

  /// Minimal PactPluginRpc implementation that records the init request it received.
  pub(crate) struct InitRecordingPlugin {
    pub request: std::sync::RwLock<Option<crate::plugin_models::PluginInitRequest>>,
  }

  impl Default for InitRecordingPlugin {
    fn default() -> Self {
      Self {
        request: std::sync::RwLock::new(None),
      }
    }
  }

  #[async_trait]
  impl crate::plugin_models::PactPluginRpc for InitRecordingPlugin {
    async fn init_plugin(
      &mut self,
      request: crate::plugin_models::PluginInitRequest,
    ) -> anyhow::Result<PluginInitResponse> {
      *self.request.write().unwrap() = Some(request);
      Ok(PluginInitResponse {
        catalogue: vec![],
        plugin_capabilities: vec!["interaction/request-response".to_string()],
      })
    }
  }
}
