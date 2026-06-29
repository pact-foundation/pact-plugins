//! gRPC-based plugin implementation

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::anyhow;
use async_trait::async_trait;
use log::max_level;
use os_info::Type;
use prost::Message;
use sysinfo::{Pid, System};
use tonic::codegen::InterceptedService;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::Interceptor;
use tonic::transport::Channel;
use tonic::{Request, Status};
use tracing::{debug, trace, warn};
use uuid::Uuid;

use crate::child_process::ChildPluginProcess;
use crate::plugin_manager::{deregister_plugin_instance, register_plugin_instance};
use crate::plugin_models::{
  PactPlugin, PactPluginManifest, PactPluginRpc, PluginInitRequest, PluginInitResponse,
  PluginInterfaceVersion,
};
use crate::proto::pact_plugin_client::PactPluginClient as PactPluginClientV1;
use crate::proto::*;
use crate::proto_v2::{self, pact_plugin_client::PactPluginClient as PactPluginClientV2};

// ---------------------------------------------------------------------------
// PluginClient — V1/V2 gRPC client selector
// ---------------------------------------------------------------------------

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
        if let Some(id) = crate::test_context::current_test_run_id() {
          let ctx = v2_req.test_context.get_or_insert_with(prost_types::Struct::default);
          ctx.fields.entry("testRunId".to_string()).or_insert_with(|| prost_types::Value {
            kind: Some(prost_types::value::Kind::StringValue(id)),
          });
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
        if let Some(id) = crate::test_context::current_test_run_id() {
          let ctx = v2_req.test_context.get_or_insert_with(prost_types::Struct::default);
          ctx.fields.entry("testRunId".to_string()).or_insert_with(|| prost_types::Value {
            kind: Some(prost_types::value::Kind::StringValue(id)),
          });
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

// ---------------------------------------------------------------------------
// PactPluginInterceptor — injects server key as auth header
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) struct PactPluginInterceptor {
  server_key: MetadataValue<Ascii>,
}

impl PactPluginInterceptor {
  pub(crate) fn new(server_key: &str) -> anyhow::Result<Self> {
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

// ---------------------------------------------------------------------------
// GrpcPactPlugin — concrete gRPC-backed plugin instance
// ---------------------------------------------------------------------------

/// Running gRPC plugin details
#[derive(Debug, Clone)]
pub struct GrpcPactPlugin {
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

impl GrpcPactPlugin {
  /// Create a new GrpcPactPlugin
  pub fn new(manifest: &PactPluginManifest, child: ChildPluginProcess) -> anyhow::Result<Self> {
    let instance_id = child.instance_id.clone();
    Ok(GrpcPactPlugin {
      manifest: manifest.clone(),
      interface_version: PluginInterfaceVersion::try_from(manifest.plugin_interface_version)?,
      instance_id,
      child: Arc::new(child),
      plugin_capabilities: vec![],
      access_count: Arc::new(AtomicUsize::new(1)),
    })
  }

  /// Port the plugin is running on
  pub fn port(&self) -> u16 {
    self.child.port()
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

#[async_trait]
impl PactPlugin for GrpcPactPlugin {
  fn manifest(&self) -> &PactPluginManifest {
    &self.manifest
  }

  fn kill(&self) {
    self.child.kill();
  }

  fn update_access(&self) {
    let count = self.access_count.fetch_add(1, Ordering::SeqCst);
    trace!(
      "update_access: Plugin {}/{} access is now {}",
      self.manifest.name,
      self.manifest.version,
      count + 1
    );
  }

  fn drop_access(&self) -> usize {
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

  fn instance_id(&self) -> &str {
    &self.instance_id
  }

  fn has_capability(&self, capability: &str) -> bool {
    self.plugin_capabilities.iter().any(|c| c == capability)
  }

  async fn compare_contents(
    &self,
    request: CompareContentsRequest,
  ) -> anyhow::Result<CompareContentsResponse> {
    let mut client = self.get_plugin_client().await?;
    client.compare_contents(request).await.map_err(anyhow::Error::from)
  }

  async fn configure_interaction(
    &self,
    request: ConfigureInteractionRequest,
  ) -> anyhow::Result<ConfigureInteractionResponse> {
    let mut client = self.get_plugin_client().await?;
    client.configure_interaction(request).await.map_err(anyhow::Error::from)
  }

  async fn generate_content(
    &self,
    request: GenerateContentRequest,
  ) -> anyhow::Result<GenerateContentResponse> {
    let mut client = self.get_plugin_client().await?;
    client.generate_content(request).await.map_err(anyhow::Error::from)
  }

  async fn start_mock_server(
    &self,
    request: StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse> {
    let mut client = self.get_plugin_client().await?;
    client.start_mock_server(request).await.map_err(anyhow::Error::from)
  }

  async fn start_mock_server_v2(
    &self,
    request: proto_v2::StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse> {
    let mut client = self.get_plugin_client().await?;
    client.start_mock_server_v2(request).await.map_err(anyhow::Error::from)
  }

  async fn shutdown_mock_server(
    &self,
    request: ShutdownMockServerRequest,
  ) -> anyhow::Result<ShutdownMockServerResponse> {
    let mut client = self.get_plugin_client().await?;
    client.shutdown_mock_server(request).await.map_err(anyhow::Error::from)
  }

  async fn get_mock_server_results(
    &self,
    request: MockServerRequest,
  ) -> anyhow::Result<MockServerResults> {
    let mut client = self.get_plugin_client().await?;
    client.get_mock_server_results(request).await.map_err(anyhow::Error::from)
  }

  async fn prepare_interaction_for_verification(
    &self,
    request: VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse> {
    let mut client = self.get_plugin_client().await?;
    client.prepare_interaction_for_verification(request).await.map_err(anyhow::Error::from)
  }

  async fn prepare_interaction_for_verification_v2(
    &self,
    request: proto_v2::VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse> {
    let mut client = self.get_plugin_client().await?;
    client.prepare_interaction_for_verification_v2(request).await.map_err(anyhow::Error::from)
  }

  async fn verify_interaction(
    &self,
    request: VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse> {
    let mut client = self.get_plugin_client().await?;
    client.verify_interaction(request).await.map_err(anyhow::Error::from)
  }

  async fn verify_interaction_v2(
    &self,
    request: proto_v2::VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse> {
    let mut client = self.get_plugin_client().await?;
    client.verify_interaction_v2(request).await.map_err(anyhow::Error::from)
  }

  async fn update_catalogue(&self, request: Catalogue) -> anyhow::Result<()> {
    let mut client = self.get_plugin_client().await?;
    client.update_catalogue(request).await.map_err(anyhow::Error::from)
  }
}

#[async_trait]
impl PactPluginRpc for GrpcPactPlugin {
  async fn init_plugin(
    &mut self,
    request: PluginInitRequest,
  ) -> anyhow::Result<PluginInitResponse> {
    let mut client = self.get_plugin_client().await?;
    client.init_plugin(request).await.map_err(anyhow::Error::from)
  }
}

// ---------------------------------------------------------------------------
// start_plugin_process — launches the plugin executable
// ---------------------------------------------------------------------------

pub(crate) async fn start_plugin_process(
  manifest: &PactPluginManifest,
) -> anyhow::Result<GrpcPactPlugin> {
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

  let host_port = match crate::plugin_host::ensure_plugin_host_running().await {
    Ok(port) => Some(port),
    Err(err) => {
      warn!("Could not start PluginHost server, Log RPC forwarding will be unavailable: {}", err);
      None
    }
  };

  let log_level = max_level();
  let mut child_command = Command::new(path.clone());
  let mut child_command = child_command
    .env("LOG_LEVEL", log_level.to_string())
    .env("RUST_LOG", log_level.to_string())
    .current_dir(manifest.plugin_dir.clone());

  let instance_id = Uuid::new_v4().to_string();

  child_command = child_command.env("PACT_PLUGIN_INSTANCE_ID", &instance_id);
  if let Some(port) = host_port {
    child_command = child_command.env("PACT_PLUGIN_HOST", format!("127.0.0.1:{}", port));
  }

  if let Some(args) = &manifest.args {
    child_command = child_command.args(args);
  }

  let child = child_command
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| {
      anyhow!(
        "Was not able to start plugin process for '{}' - {}",
        path.to_string_lossy(),
        err
      )
    })?;
  let child_pid = child.id();
  debug!("Plugin {} started with PID {} (instance {})", manifest.name, child_pid, instance_id);
  register_plugin_instance(&instance_id, &manifest.name);

  match ChildPluginProcess::new(child, manifest, instance_id.clone()).await {
    Ok(child) => {
      let plugin = GrpcPactPlugin::new(manifest, child)?;
      Ok(plugin)
    }
    Err(err) => {
      deregister_plugin_instance(&instance_id);
      let mut s = System::new();
      s.refresh_processes();
      if let Some(process) = s.process(Pid::from_u32(child_pid)) {
        #[cfg(not(windows))]
        process.kill();
        #[cfg(windows)]
        let _ = Command::new("taskkill.exe")
          .arg("/PID")
          .arg(child_pid.to_string())
          .arg("/F")
          .arg("/T")
          .output();
      } else {
        warn!("Child process with PID {} was not found", child_pid);
      }
      Err(err)
    }
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod tests {
  use std::collections::HashMap;

  use tonic::Status;

  use crate::grpc_plugin::PluginClient;
  use crate::plugin_models::{PluginInitRequest, PluginInitResponse};
  use crate::proto::*;
  use crate::proto_v2;

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
}
