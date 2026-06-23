//! Models for representing plugins

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::anyhow;
use async_trait::async_trait;
use prost::Message;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tonic::codegen::InterceptedService;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::Interceptor;
use tonic::transport::Channel;
use tonic::{Request, Status};
use tracing::{debug, trace};

use crate::child_process::ChildPluginProcess;
use crate::proto::pact_plugin_client::PactPluginClient as PactPluginClientV1;
use crate::proto::*;
use crate::proto_v2::{self, pact_plugin_client::PactPluginClient as PactPluginClientV2};

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

enum PluginClient {
  V1(PactPluginClientV1<InterceptedService<Channel, PactPluginInterceptor>>),
  V2(PactPluginClientV2<InterceptedService<Channel, PactPluginInterceptor>>),
}

impl PluginClient {
  fn convert_message<T, U>(message: T) -> Result<U, Status>
  where
    T: Message,
    U: Message + Default,
  {
    U::decode(message.encode_to_vec().as_slice()).map_err(|err| {
      Status::internal(format!(
        "Failed to convert between plugin interface message versions: {}",
        err
      ))
    })
  }

  async fn init_plugin(
    &mut self,
    request: PluginInitRequest,
  ) -> Result<PluginInitResponse, Status> {
    match self {
      PluginClient::V1(client) => client
        .init_plugin(Request::new(InitPluginRequest {
          implementation: request.implementation,
          version: request.version,
        }))
        .await
        .map(|response| PluginInitResponse {
          catalogue: response.into_inner().catalogue,
          plugin_capabilities: vec![],
        }),
      PluginClient::V2(client) => client
        .init_plugin(Request::new(proto_v2::InitPluginRequest {
          implementation: request.implementation,
          version: request.version,
          host_capabilities: request.host_capabilities,
          plugin_instance_id: request.plugin_instance_id,
        }))
        .await
        .and_then(|response| match response.into_inner().response {
          Some(proto_v2::init_plugin_response::Response::Success(success)) => {
            Ok(PluginInitResponse {
              catalogue: success
                .catalogue
                .into_iter()
                .map(Self::convert_message)
                .collect::<Result<Vec<CatalogueEntry>, Status>>()?,
              plugin_capabilities: success.plugin_capabilities,
            })
          }
          Some(proto_v2::init_plugin_response::Response::Failure(failure)) => {
            let mut error = failure.error;
            if !failure.missing_host_capabilities.is_empty() {
              error.push_str(" (missing host capabilities: ");
              error.push_str(failure.missing_host_capabilities.join(", ").as_str());
              error.push(')');
            }
            Err(Status::failed_precondition(error))
          }
          None => Err(Status::internal(
            "Plugin returned an invalid V2 InitPlugin response",
          )),
        }),
    }
  }

  async fn compare_contents(
    &mut self,
    request: CompareContentsRequest,
  ) -> Result<CompareContentsResponse, Status> {
    match self {
      PluginClient::V1(client) => client
        .compare_contents(Request::new(request))
        .await
        .map(|response| response.into_inner()),
      PluginClient::V2(client) => {
        let mut v2_req = Self::convert_message::<_, proto_v2::CompareContentsRequest>(request)?;
        if v2_req.test_context.is_none() {
          v2_req.test_context = crate::test_context::current_test_context();
        }
        client
          .compare_contents(Request::new(v2_req))
          .await
          .and_then(|response| Self::convert_message(response.into_inner()))
      }
    }
  }

  async fn configure_interaction(
    &mut self,
    request: ConfigureInteractionRequest,
  ) -> Result<ConfigureInteractionResponse, Status> {
    match self {
      PluginClient::V1(client) => client
        .configure_interaction(Request::new(request))
        .await
        .map(|response| response.into_inner()),
      PluginClient::V2(client) => {
        let mut v2_req =
          Self::convert_message::<_, proto_v2::ConfigureInteractionRequest>(request)?;
        if v2_req.test_context.is_none() {
          v2_req.test_context = crate::test_context::current_test_context();
        }
        client
          .configure_interaction(Request::new(v2_req))
          .await
          .and_then(|response| Self::convert_message(response.into_inner()))
      }
    }
  }

  async fn generate_content(
    &mut self,
    request: GenerateContentRequest,
  ) -> Result<GenerateContentResponse, Status> {
    match self {
      PluginClient::V1(client) => client
        .generate_content(Request::new(request))
        .await
        .map(|response| response.into_inner()),
      PluginClient::V2(client) => client
        .generate_content(Request::new(Self::convert_message::<
          _,
          proto_v2::GenerateContentRequest,
        >(request)?))
        .await
        .and_then(|response| Self::convert_message(response.into_inner())),
    }
  }

  async fn start_mock_server(
    &mut self,
    request: StartMockServerRequest,
  ) -> Result<StartMockServerResponse, Status> {
    match self {
      PluginClient::V1(client) => client
        .start_mock_server(Request::new(request))
        .await
        .map(|response| response.into_inner()),
      PluginClient::V2(client) => client
        .start_mock_server(Request::new(Self::convert_message::<
          _,
          proto_v2::StartMockServerRequest,
        >(request)?))
        .await
        .and_then(|response| Self::convert_message(response.into_inner())),
    }
  }

  async fn shutdown_mock_server(
    &mut self,
    request: ShutdownMockServerRequest,
  ) -> Result<ShutdownMockServerResponse, Status> {
    match self {
      PluginClient::V1(client) => client
        .shutdown_mock_server(Request::new(request))
        .await
        .map(|response| response.into_inner()),
      PluginClient::V2(client) => client
        .shutdown_mock_server(Request::new(Self::convert_message::<
          _,
          proto_v2::MockServerRequest,
        >(request)?))
        .await
        .and_then(|response| Self::convert_message::<_, ShutdownMockServerResponse>(response.into_inner())),
    }
  }

  async fn start_mock_server_v2(
    &mut self,
    request: proto_v2::StartMockServerRequest,
  ) -> Result<StartMockServerResponse, Status> {
    match self {
      PluginClient::V1(_) => Err(Status::unimplemented("V2 interface not supported on V1 plugin")),
      PluginClient::V2(client) => client
        .start_mock_server(Request::new(request))
        .await
        .and_then(|response| Self::convert_message(response.into_inner())),
    }
  }

  async fn prepare_interaction_for_verification_v2(
    &mut self,
    request: proto_v2::VerificationPreparationRequest,
  ) -> Result<VerificationPreparationResponse, Status> {
    match self {
      PluginClient::V1(_) => Err(Status::unimplemented("V2 interface not supported on V1 plugin")),
      PluginClient::V2(client) => client
        .prepare_interaction_for_verification(Request::new(request))
        .await
        .and_then(|response| Self::convert_message(response.into_inner())),
    }
  }

  async fn verify_interaction_v2(
    &mut self,
    request: proto_v2::VerifyInteractionRequest,
  ) -> Result<VerifyInteractionResponse, Status> {
    match self {
      PluginClient::V1(_) => Err(Status::unimplemented("V2 interface not supported on V1 plugin")),
      PluginClient::V2(client) => client
        .verify_interaction(Request::new(request))
        .await
        .and_then(|response| Self::convert_message(response.into_inner())),
    }
  }

  async fn get_mock_server_results(
    &mut self,
    request: MockServerRequest,
  ) -> Result<MockServerResults, Status> {
    match self {
      PluginClient::V1(client) => client
        .get_mock_server_results(Request::new(request))
        .await
        .map(|response| response.into_inner()),
      PluginClient::V2(client) => client
        .get_mock_server_results(Request::new(Self::convert_message::<
          _,
          proto_v2::MockServerRequest,
        >(request)?))
        .await
        .and_then(|response| Self::convert_message(response.into_inner())),
    }
  }

  async fn prepare_interaction_for_verification(
    &mut self,
    request: VerificationPreparationRequest,
  ) -> Result<VerificationPreparationResponse, Status> {
    match self {
      PluginClient::V1(client) => client
        .prepare_interaction_for_verification(Request::new(request))
        .await
        .map(|response| response.into_inner()),
      PluginClient::V2(client) => client
        .prepare_interaction_for_verification(Request::new(Self::convert_message::<
          _,
          proto_v2::VerificationPreparationRequest,
        >(request)?))
        .await
        .and_then(|response| Self::convert_message(response.into_inner())),
    }
  }

  async fn verify_interaction(
    &mut self,
    request: VerifyInteractionRequest,
  ) -> Result<VerifyInteractionResponse, Status> {
    match self {
      PluginClient::V1(client) => client
        .verify_interaction(Request::new(request))
        .await
        .map(|response| response.into_inner()),
      PluginClient::V2(client) => client
        .verify_interaction(Request::new(Self::convert_message::<
          _,
          proto_v2::VerifyInteractionRequest,
        >(request)?))
        .await
        .and_then(|response| Self::convert_message(response.into_inner())),
    }
  }

  async fn update_catalogue(&mut self, request: Catalogue) -> Result<(), Status> {
    match self {
      PluginClient::V1(client) => client
        .update_catalogue(Request::new(request))
        .await
        .map(|_| ()),
      PluginClient::V2(client) => client
        .update_catalogue(Request::new(
          Self::convert_message::<_, proto_v2::Catalogue>(request)?,
        ))
        .await
        .map(|_| ()),
    }
  }
}

/// Trait with remote-calling methods for a running plugin
#[async_trait]
pub trait PactPluginRpc {
  /// Send an init request to the plugin process
  async fn init_plugin(&mut self, request: PluginInitRequest)
    -> anyhow::Result<PluginInitResponse>;

  /// Send a compare contents request to the plugin process
  async fn compare_contents(
    &self,
    request: CompareContentsRequest,
  ) -> anyhow::Result<CompareContentsResponse>;

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

  /// Start a mock server
  async fn start_mock_server(
    &self,
    request: StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse>;

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

  /// Prepare an interaction for verification. This should return any data required to construct any request
  /// so that it can be amended before the verification is run.
  async fn prepare_interaction_for_verification(
    &self,
    request: VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse>;

  /// Execute the verification for the interaction.
  async fn verify_interaction(
    &self,
    request: VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse>;

  /// Updates the catalogue. This will be sent when the core catalogue has been updated (probably by a plugin loading).
  async fn update_catalogue(&self, request: Catalogue) -> anyhow::Result<()>;

  /// Start a mock server using V2 structured interaction data (no pact JSON).
  async fn start_mock_server_v2(
    &self,
    request: proto_v2::StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse> {
    let _ = request;
    Err(anyhow!("V2 interface not supported by this plugin"))
  }

  /// Prepare an interaction for verification using V2 structured interaction data.
  async fn prepare_interaction_for_verification_v2(
    &self,
    request: proto_v2::VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse> {
    let _ = request;
    Err(anyhow!("V2 interface not supported by this plugin"))
  }

  /// Execute the verification for the interaction using V2 structured interaction data.
  async fn verify_interaction_v2(
    &self,
    request: proto_v2::VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse> {
    let _ = request;
    Err(anyhow!("V2 interface not supported by this plugin"))
  }
}

/// Running plugin details
#[derive(Debug, Clone)]
pub struct PactPlugin {
  /// Manifest for this plugin
  pub manifest: PactPluginManifest,

  /// Interface version supported by the plugin
  pub interface_version: PluginInterfaceVersion,

  /// Running child process
  pub child: Arc<ChildPluginProcess>,

  /// Optional capabilities negotiated for this plugin instance
  pub plugin_capabilities: Vec<String>,

  /// UUID assigned by the driver at process start; used to correlate log output from this instance
  pub instance_id: String,

  /// Count of access to the plugin. If this is ever zero, the plugin process will be shutdown
  access_count: Arc<AtomicUsize>,
}

#[async_trait]
impl PactPluginRpc for PactPlugin {
  /// Send an init request to the plugin process
  async fn init_plugin(
    &mut self,
    request: PluginInitRequest,
  ) -> anyhow::Result<PluginInitResponse> {
    let mut client = self.get_plugin_client().await?;
    client
      .init_plugin(request)
      .await
      .map_err(anyhow::Error::from)
  }

  /// Send a compare contents request to the plugin process
  async fn compare_contents(
    &self,
    request: CompareContentsRequest,
  ) -> anyhow::Result<CompareContentsResponse> {
    let mut client = self.get_plugin_client().await?;
    client
      .compare_contents(request)
      .await
      .map_err(anyhow::Error::from)
  }

  /// Send a configure contents request to the plugin process
  async fn configure_interaction(
    &self,
    request: ConfigureInteractionRequest,
  ) -> anyhow::Result<ConfigureInteractionResponse> {
    let mut client = self.get_plugin_client().await?;
    client
      .configure_interaction(request)
      .await
      .map_err(anyhow::Error::from)
  }

  /// Send a generate content request to the plugin
  async fn generate_content(
    &self,
    request: GenerateContentRequest,
  ) -> anyhow::Result<GenerateContentResponse> {
    let mut client = self.get_plugin_client().await?;
    client
      .generate_content(request)
      .await
      .map_err(anyhow::Error::from)
  }

  async fn start_mock_server(
    &self,
    request: StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse> {
    let mut client = self.get_plugin_client().await?;
    client
      .start_mock_server(request)
      .await
      .map_err(anyhow::Error::from)
  }

  async fn shutdown_mock_server(
    &self,
    request: ShutdownMockServerRequest,
  ) -> anyhow::Result<ShutdownMockServerResponse> {
    let mut client = self.get_plugin_client().await?;
    client
      .shutdown_mock_server(request)
      .await
      .map_err(anyhow::Error::from)
  }

  async fn get_mock_server_results(
    &self,
    request: MockServerRequest,
  ) -> anyhow::Result<MockServerResults> {
    let mut client = self.get_plugin_client().await?;
    client
      .get_mock_server_results(request)
      .await
      .map_err(anyhow::Error::from)
  }

  async fn prepare_interaction_for_verification(
    &self,
    request: VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse> {
    let mut client = self.get_plugin_client().await?;
    client
      .prepare_interaction_for_verification(request)
      .await
      .map_err(anyhow::Error::from)
  }

  async fn verify_interaction(
    &self,
    request: VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse> {
    let mut client = self.get_plugin_client().await?;
    client
      .verify_interaction(request)
      .await
      .map_err(anyhow::Error::from)
  }

  async fn update_catalogue(&self, request: Catalogue) -> anyhow::Result<()> {
    let mut client = self.get_plugin_client().await?;
    client
      .update_catalogue(request)
      .await
      .map_err(anyhow::Error::from)
  }

  async fn start_mock_server_v2(
    &self,
    request: proto_v2::StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse> {
    let mut client = self.get_plugin_client().await?;
    client
      .start_mock_server_v2(request)
      .await
      .map_err(anyhow::Error::from)
  }

  async fn prepare_interaction_for_verification_v2(
    &self,
    request: proto_v2::VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse> {
    let mut client = self.get_plugin_client().await?;
    client
      .prepare_interaction_for_verification_v2(request)
      .await
      .map_err(anyhow::Error::from)
  }

  async fn verify_interaction_v2(
    &self,
    request: proto_v2::VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse> {
    let mut client = self.get_plugin_client().await?;
    client
      .verify_interaction_v2(request)
      .await
      .map_err(anyhow::Error::from)
  }
}

impl PactPlugin {
  /// Create a new Plugin
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
    trace!(
      "update_access: Plugin {}/{} access is now {}",
      self.manifest.name,
      self.manifest.version,
      count + 1
    );
  }

  /// Decrement and return the access count for the plugin
  pub fn drop_access(&mut self) -> usize {
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

  async fn connect_channel(&self) -> anyhow::Result<Channel> {
    let port = self.child.port();
    match Channel::from_shared(format!("http://[::1]:{}", port))?
      .connect()
      .await
    {
      Ok(channel) => Ok(channel),
      Err(err) => {
        debug!("IP6 connection failed, will try IP4 address - {err}");
        Channel::from_shared(format!("http://127.0.0.1:{}", port))?
          .connect()
          .await
          .map_err(|err| anyhow!(err))
      }
    }
  }

  async fn get_plugin_client(&self) -> anyhow::Result<PluginClient> {
    let channel = self.connect_channel().await?;
    let interceptor = PactPluginInterceptor::new(self.child.plugin_info.server_key.as_str())?;
    match self.interface_version {
      PluginInterfaceVersion::V1 => Ok(PluginClient::V1(PactPluginClientV1::with_interceptor(
        channel,
        interceptor,
      ))),
      PluginInterfaceVersion::V2 => Ok(PluginClient::V2(PactPluginClientV2::with_interceptor(
        channel,
        interceptor,
      ))),
    }
  }
}

/// Interceptor to inject the server key as an authorisation header
#[derive(Clone, Debug)]
struct PactPluginInterceptor {
  /// Server key to inject
  server_key: MetadataValue<Ascii>,
}

impl PactPluginInterceptor {
  fn new(server_key: &str) -> anyhow::Result<Self> {
    let token = MetadataValue::try_from(server_key)?;
    Ok(PactPluginInterceptor { server_key: token })
  }
}

impl Interceptor for PactPluginInterceptor {
  fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
    request
      .metadata_mut()
      .insert("authorization", self.server_key.clone());
    Ok(request)
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
  use std::collections::HashMap;
  use std::sync::RwLock;

  use async_trait::async_trait;
  use tonic::Status;

  use crate::plugin_models::{
    PactPluginRpc, PluginClient, PluginInitRequest, PluginInitResponse,
  };
  use crate::proto::verification_preparation_response::Response;
  use crate::proto::*;
  use crate::proto_v2;

  pub(crate) struct MockPlugin {
    pub prepare_request: RwLock<VerificationPreparationRequest>,
    pub verify_request: RwLock<VerifyInteractionRequest>,
  }

  impl Default for MockPlugin {
    fn default() -> Self {
      MockPlugin {
        prepare_request: RwLock::new(VerificationPreparationRequest::default()),
        verify_request: RwLock::new(VerifyInteractionRequest::default()),
      }
    }
  }

  #[test]
  fn converts_between_v1_and_v2_messages() {
    let request = PluginInitRequest {
      implementation: "plugin-driver-rust".to_string(),
      version: "1.0.0-beta.1".to_string(),
      host_capabilities: vec!["interaction/request-response".to_string()],
      plugin_instance_id: "test-instance-id".to_string(),
    };

    let converted_request = proto_v2::InitPluginRequest {
      implementation: request.implementation,
      version: request.version,
      host_capabilities: request.host_capabilities,
      plugin_instance_id: request.plugin_instance_id,
    };
    assert_eq!(converted_request.implementation, "plugin-driver-rust");
    assert_eq!(converted_request.version, "1.0.0-beta.1");
    assert_eq!(
      converted_request.host_capabilities,
      vec!["interaction/request-response"]
    );

    let response = proto_v2::InitPluginResponse {
      response: Some(proto_v2::init_plugin_response::Response::Success(
        proto_v2::InitPluginSuccess {
          catalogue: vec![proto_v2::CatalogueEntry {
            r#type: proto_v2::catalogue_entry::EntryType::ContentMatcher as i32,
            key: "test".to_string(),
            values: HashMap::new(),
          }],
          plugin_capabilities: vec!["plugin/verification".to_string()],
        },
      )),
    };

    let converted_response = match response.response.unwrap() {
      proto_v2::init_plugin_response::Response::Success(success) => PluginInitResponse {
        catalogue: success
          .catalogue
          .into_iter()
          .map(PluginClient::convert_message)
          .collect::<Result<Vec<CatalogueEntry>, Status>>()
          .unwrap(),
        plugin_capabilities: success.plugin_capabilities,
      },
      _ => unreachable!(),
    };
    assert_eq!(converted_response.catalogue.len(), 1);
    assert_eq!(converted_response.catalogue[0].key, "test");
    assert_eq!(
      converted_response.catalogue[0].r#type,
      catalogue_entry::EntryType::ContentMatcher as i32
    );
    assert_eq!(converted_response.plugin_capabilities, vec!["plugin/verification"]);
  }

  #[async_trait]
  impl PactPluginRpc for MockPlugin {
    async fn init_plugin(
      &mut self,
      _request: PluginInitRequest,
    ) -> anyhow::Result<PluginInitResponse> {
      unimplemented!()
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
}
