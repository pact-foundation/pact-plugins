//! Models for representing plugins

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tonic::{Request, Status};
use tonic::codegen::InterceptedService;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::Interceptor;
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
      args: None,
      dependencies: None,
      plugin_config: Default::default()
    }
  }
}

/// Trait with remote-calling methods for a running plugin
#[async_trait]
pub trait PactPluginRpc {
  /// Send an init request to the plugin process
  async fn init_plugin(&mut self, request: InitPluginRequest) -> anyhow::Result<InitPluginResponse>;

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

  /// Updates the catalogue. This will be sent when the core catalogue has been updated (probably by a plugin loading).
  async fn update_catalogue(&self, request: Catalogue) -> anyhow::Result<()>;
}

/// Running plugin details
#[derive(Debug, Clone)]
pub struct PactPlugin {
  /// Manifest for this plugin
  pub manifest: PactPluginManifest,

  /// Running child process
  pub child: Arc<ChildPluginProcess>,

  /// Count of access to the plugin. If this is ever zero, the plugin process will be shutdown
  access_count: Arc<AtomicUsize>
}

#[async_trait]
impl PactPluginRpc for PactPlugin {
  /// Send an init request to the plugin process
  async fn init_plugin(&mut self, request: InitPluginRequest) -> anyhow::Result<InitPluginResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.init_plugin(Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  /// Send a compare contents request to the plugin process
  async fn compare_contents(&self, request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.compare_contents(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  /// Send a configure contents request to the plugin process
  async fn configure_interaction(&self, request: ConfigureInteractionRequest) -> anyhow::Result<ConfigureInteractionResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.configure_interaction(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  /// Send a generate content request to the plugin
  async fn generate_content(&self, request: GenerateContentRequest) -> anyhow::Result<GenerateContentResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.generate_content(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn start_mock_server(&self, request: StartMockServerRequest) -> anyhow::Result<StartMockServerResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.start_mock_server(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn shutdown_mock_server(&self, request: ShutdownMockServerRequest) -> anyhow::Result<ShutdownMockServerResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.shutdown_mock_server(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn get_mock_server_results(&self, request: MockServerRequest) -> anyhow::Result<MockServerResults> {
    let mut client = self.get_plugin_client().await?;
    let response = client.get_mock_server_results(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn prepare_interaction_for_verification(&self, request: VerificationPreparationRequest) -> anyhow::Result<VerificationPreparationResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.prepare_interaction_for_verification(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn verify_interaction(&self, request: VerifyInteractionRequest) -> anyhow::Result<VerifyInteractionResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.verify_interaction(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn update_catalogue(&self, request: Catalogue) -> anyhow::Result<()> {
    let mut client = self.get_plugin_client().await?;
    client.update_catalogue(tonic::Request::new(request)).await?;
    Ok(())
  }
}

impl PactPlugin {
  /// Create a new Plugin
  pub fn new(manifest: &PactPluginManifest, child: ChildPluginProcess) -> Self {
    PactPlugin {
      manifest: manifest.clone(),
      child: Arc::new(child),
      access_count: Arc::new(AtomicUsize::new(1))
    }
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
    let count = self.access_count.fetch_add(1, Ordering::SeqCst);
    trace!("update_access: Plugin {}/{} access is now {}", self.manifest.name,
      self.manifest.version, count + 1);
  }

  /// Decrement and return the access count for the plugin
  pub fn drop_access(&mut self) -> usize {
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

  async fn get_plugin_client(&self) -> anyhow::Result<PactPluginClient<InterceptedService<Channel, PactPluginInterceptor>>> {
    let channel = self.connect_channel().await?;
    let interceptor = PactPluginInterceptor::new(self.child.plugin_info.server_key.as_str())?;
    Ok(PactPluginClient::with_interceptor(channel, interceptor))
  }
}

/// Interceptor to inject the server key as an authorisation header
#[derive(Clone, Debug)]
struct PactPluginInterceptor {
  /// Server key to inject
  server_key: MetadataValue<Ascii>
}

impl PactPluginInterceptor {
  fn new(server_key: &str) -> anyhow::Result<Self> {
    let token = MetadataValue::try_from(server_key)?;
    Ok(PactPluginInterceptor {
      server_key: token
    })
  }
}

impl Interceptor for PactPluginInterceptor {
  fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
    request.metadata_mut().insert("authorization", self.server_key.clone());
    Ok(request)
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

#[cfg(test)]
pub(crate) mod tests {
  use async_trait::async_trait;
  use lazy_static::lazy_static;
  use std::sync::RwLock;

  use crate::plugin_models::PactPluginRpc;
  use crate::proto::*;
  use crate::proto::verification_preparation_response::Response;

  lazy_static!{
     pub(crate) static ref PREPARE_INTERACTION_FOR_VERIFICATION_ARG: RwLock<Option<VerificationPreparationRequest>> = RwLock::new(None);
     pub(crate) static ref VERIFY_INTERACTION_ARG: RwLock<Option<VerifyInteractionRequest>> = RwLock::new(None);
  }

  #[derive(Default)]
  pub(crate) struct MockPlugin {}

  #[async_trait]
  impl PactPluginRpc for MockPlugin {
    async fn init_plugin(&mut self, _request: InitPluginRequest) -> anyhow::Result<InitPluginResponse> {
      unimplemented!()
    }

    async fn compare_contents(&self, _request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse> {
      unimplemented!()
    }

    async fn configure_interaction(&self, _request: ConfigureInteractionRequest) -> anyhow::Result<ConfigureInteractionResponse> {
      unimplemented!()
    }

    async fn generate_content(&self, _request: GenerateContentRequest) -> anyhow::Result<GenerateContentResponse> {
      unimplemented!()
    }

    async fn start_mock_server(&self, _request: StartMockServerRequest) -> anyhow::Result<StartMockServerResponse> {
      unimplemented!()
    }

    async fn shutdown_mock_server(&self, _request: ShutdownMockServerRequest) -> anyhow::Result<ShutdownMockServerResponse> {
      unimplemented!()
    }

    async fn get_mock_server_results(&self, _request: MockServerRequest) -> anyhow::Result<MockServerResults> {
      unimplemented!()
    }

    async fn prepare_interaction_for_verification(&self, request: VerificationPreparationRequest) -> anyhow::Result<VerificationPreparationResponse> {
      let mut w = PREPARE_INTERACTION_FOR_VERIFICATION_ARG.write().unwrap();
      let _ = w.insert(request);
      let data = InteractionData {
        body: None,
        metadata: Default::default()
      };
      Ok(VerificationPreparationResponse {
        response: Some(Response::InteractionData(data))
      })
    }

    async fn verify_interaction(&self, request: VerifyInteractionRequest) -> anyhow::Result<VerifyInteractionResponse> {
      let mut w = VERIFY_INTERACTION_ARG.write().unwrap();
      let _ = w.insert(request);
      let result = VerificationResult {
        success: false,
        response_data: None,
        mismatches: vec![],
        output: vec![]
      };
      Ok(VerifyInteractionResponse {
        response: Some(verify_interaction_response::Response::Result(result))
      })
    }

    async fn update_catalogue(&self, _request: Catalogue) -> anyhow::Result<()> {
      unimplemented!()
    }
  }
}
