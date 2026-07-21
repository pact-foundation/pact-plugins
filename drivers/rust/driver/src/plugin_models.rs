//! Models for representing plugins

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::trace;

use crate::child_process::ChildPluginProcess;
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

/// Trait for the plugin init handshake only (used by anything that can handle the init message)
#[async_trait]
pub trait PactPluginRpc {
  /// Send an init request to the plugin process
  async fn init_plugin(&mut self, request: PluginInitRequest)
    -> anyhow::Result<PluginInitResponse>;
}

/// Trait for a running plugin instance.
///
/// Implementations include [`crate::grpc_plugin::GrpcPactPlugin`] for exec-type plugins
/// that communicate via gRPC, and future embedded runtimes (Lua, Python, …).
#[async_trait]
pub trait PluginInstance: std::fmt::Debug + Send + Sync {
  /// Return the manifest for this plugin.
  fn manifest(&self) -> &PactPluginManifest;

  /// Return the instance ID assigned to this plugin at startup.
  fn instance_id(&self) -> &str;

  /// Check whether the plugin declared a specific capability.
  fn has_capability(&self, capability: &str) -> bool;

  /// Terminate the running plugin. The default no-op suits embedded runtimes
  /// that are not managed as a child process.
  fn kill(&self) {}

  /// Send a compare contents request to the plugin process
  async fn compare_contents(
    &self,
    request: CompareContentsRequest,
  ) -> anyhow::Result<CompareContentsResponse>;

  /// Send a compare contents request to the plugin process, propagating call-chain cycle
  /// detection and deadline metadata (see [`crate::call_chain`]) for transports that support it.
  /// The default implementation ignores `chain_id`/`deadline_ms` and delegates to
  /// [`PluginInstance::compare_contents`], which suits in-process runtimes (Lua, WASM) where a
  /// cycle is already caught by the native call stack; [`crate::grpc_plugin::GrpcPactPlugin`]
  /// overrides this to send the metadata over gRPC.
  async fn compare_contents_with_chain(
    &self,
    request: CompareContentsRequest,
    chain_id: &str,
    deadline_ms: u64,
  ) -> anyhow::Result<CompareContentsResponse> {
    let _ = (chain_id, deadline_ms);
    self.compare_contents(request).await
  }

  /// Send a configure contents request to the plugin process
  async fn configure_interaction(
    &self,
    request: ConfigureInteractionRequest,
  ) -> anyhow::Result<ConfigureInteractionResponse>;

  /// Send a generate content request to the plugin
  async fn generate_content(
    &self,
    request: GenerateContentRequest,
  ) -> anyhow::Result<GenerateContentResponse>;

  /// Send a generate content request to the plugin, propagating call-chain cycle detection and
  /// deadline metadata (see [`crate::call_chain`]) for transports that support it. See
  /// [`PluginInstance::compare_contents_with_chain`] for the default/override split.
  async fn generate_content_with_chain(
    &self,
    request: GenerateContentRequest,
    chain_id: &str,
    deadline_ms: u64,
  ) -> anyhow::Result<GenerateContentResponse> {
    let _ = (chain_id, deadline_ms);
    self.generate_content(request).await
  }

  /// Start a mock server
  async fn start_mock_server(
    &self,
    request: StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse>;

  /// Start a mock server using V2 structured interaction data (no pact JSON).
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

  /// Prepare an interaction for verification.
  async fn prepare_interaction_for_verification(
    &self,
    request: VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse>;

  /// Prepare an interaction for verification using V2 structured interaction data.
  async fn prepare_interaction_for_verification_v2(
    &self,
    request: proto_v2::VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse> {
    let _ = request;
    Err(anyhow!("V2 interface not supported by this plugin"))
  }

  /// Execute the verification for the interaction.
  async fn verify_interaction(
    &self,
    request: VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse>;

  /// Execute the verification for the interaction using V2 structured interaction data.
  async fn verify_interaction_v2(
    &self,
    request: proto_v2::VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse> {
    let _ = request;
    Err(anyhow!("V2 interface not supported by this plugin"))
  }

  /// Updates the catalogue.
  async fn update_catalogue(&self, request: Catalogue) -> anyhow::Result<()>;
}

/// Running plugin details
#[derive(Debug, Clone)]
pub struct PactPlugin {
  /// Manifest for this plugin
  pub manifest: PactPluginManifest,

  /// Interface version supported by the plugin
  pub interface_version: PluginInterfaceVersion,

  /// Running child process.
  ///
  /// Deprecated: not all plugin types have a child process; this field is now
  /// owned by the gRPC layer. Access plugin lifecycle through [`PluginInstance`] methods instead.
  #[deprecated(
    note = "Not all plugin types have a child process; use PluginInstance methods for plugin lifecycle"
  )]
  pub child: Arc<ChildPluginProcess>,

  /// Optional capabilities negotiated for this plugin instance
  pub plugin_capabilities: Vec<String>,

  /// UUID assigned by the driver at process start; used to correlate log output from this instance
  pub instance_id: String,

  /// Count of access to the plugin. If this is ever zero, the plugin process will be shutdown
  access_count: Arc<AtomicUsize>,
}

impl PactPlugin {
  /// Create a new Plugin
  #[allow(deprecated)]
  pub fn new(manifest: &PactPluginManifest, child: ChildPluginProcess) -> anyhow::Result<Self> {
    let instance_id = child.instance_id.clone();
    Ok(PactPlugin {
      manifest: manifest.clone(),
      interface_version: PluginInterfaceVersion::try_from(manifest.plugin_interface_version)?,
      instance_id,
      child: Arc::new(child),
      plugin_capabilities: vec![],
      access_count: Arc::new(AtomicUsize::new(1)),
    })
  }

  pub fn has_plugin_capability(&self, capability: &str) -> bool {
    self.plugin_capabilities.iter().any(|value| value == capability)
  }

  /// Check if this plugin has the given capability
  pub fn has_capability(&self, capability: &str) -> bool {
    self.plugin_capabilities.iter().any(|c| c == capability)
  }

  /// Return the instance ID for this plugin
  pub fn instance_id(&self) -> &str {
    &self.instance_id
  }

  /// Port the plugin is running on.
  ///
  /// Deprecated: port is a gRPC-specific concept; use `GrpcPactPlugin` directly if you need it.
  #[deprecated(note = "Port is specific to gRPC plugins; access it via GrpcPactPlugin")]
  #[allow(deprecated)]
  pub fn port(&self) -> u16 {
    self.child.port()
  }

  /// Kill the running plugin process.
  ///
  /// Deprecated: use [`PluginInstance::kill`] instead so non-gRPC plugin types are handled correctly.
  #[deprecated(note = "Use PluginInstance::kill() instead")]
  #[allow(deprecated)]
  pub fn kill(&self) {
    self.child.kill();
  }

  /// Update the access count of the plugin
  pub fn update_access(&self) {
    let count = self.access_count.fetch_add(1, Ordering::SeqCst);
    trace!(
      "update_access: Plugin {}/{} access is now {}",
      self.manifest.name,
      self.manifest.version,
      count + 1
    );
  }

  /// Decrement and return the access count for the plugin
  pub fn drop_access(&self) -> usize {
    let check = self
      .access_count
      .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
        if count > 0 { Some(count - 1) } else { None }
      });
    let count = if let Ok(v) = check {
      if v > 0 { v - 1 } else { v }
    } else {
      0
    };
    trace!(
      "drop_access: Plugin {}/{} access is now {}",
      self.manifest.name, self.manifest.version, count
    );
    count
  }
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

  use crate::plugin_models::{PactPluginManifest, PluginInitRequest, PluginInitResponse, PluginInstance};
  use crate::proto::verification_preparation_response::Response;
  use crate::proto::*;

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
  impl PluginInstance for MockPlugin {
    fn manifest(&self) -> &PactPluginManifest {
      &self.manifest
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

  pub(crate) struct FailingInitPlugin {
    pub error: String,
  }

  #[async_trait]
  impl crate::plugin_models::PactPluginRpc for FailingInitPlugin {
    async fn init_plugin(
      &mut self,
      _request: PluginInitRequest,
    ) -> anyhow::Result<PluginInitResponse> {
      Err(anyhow::anyhow!("{}", self.error))
    }
  }

  pub(crate) struct InitRecordingPlugin {
    pub request: RwLock<Option<PluginInitRequest>>,
  }

  impl Default for InitRecordingPlugin {
    fn default() -> Self {
      Self {
        request: RwLock::new(None),
      }
    }
  }

  #[async_trait]
  impl crate::plugin_models::PactPluginRpc for InitRecordingPlugin {
    async fn init_plugin(
      &mut self,
      request: PluginInitRequest,
    ) -> anyhow::Result<PluginInitResponse> {
      *self.request.write().unwrap() = Some(request);
      Ok(PluginInitResponse {
        catalogue: vec![],
        plugin_capabilities: vec!["interaction/request-response".to_string()],
      })
    }
  }
}
