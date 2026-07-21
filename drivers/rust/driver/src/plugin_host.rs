//! PluginHost gRPC server — receives Log RPCs from running plugins

use std::sync::OnceLock;

use anyhow::Context;
use futures_util::stream;
use tonic::metadata::MetadataMap;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

use crate::call_chain;
use crate::catalogue_manager::{CatalogueEntryType, ResolvedCapability, resolve_capability};
use crate::grpc_plugin::PluginClient;
use crate::plugin_log_sink::{PluginLogEntry, PluginLogSource, emit_plugin_log};
use crate::proto_v2::{
  CompareContentsResponse, GenerateContentResponse, HostCompareContentsRequest,
  HostGenerateContentRequest, LogMessage, plugin_host_server,
};

static PLUGIN_HOST_PORT: OnceLock<u16> = OnceLock::new();

#[derive(Debug, Default)]
struct PluginHostService;

#[tonic::async_trait]
impl plugin_host_server::PluginHost for PluginHostService {
  async fn log(&self, request: Request<LogMessage>) -> Result<Response<()>, Status> {
    let msg = request.into_inner();
    emit_plugin_log(&PluginLogEntry {
      plugin_name: crate::plugin_manager::plugin_name_for_instance(&msg.plugin_instance_id)
        .unwrap_or_default(),
      plugin_instance_id: msg.plugin_instance_id,
      test_run_id: if msg.test_run_id.is_empty() { None } else { Some(msg.test_run_id) },
      level: msg.level,
      message: msg.message,
      target: if msg.target.is_empty() { None } else { Some(msg.target) },
      timestamp_ms: msg.timestamp_ms,
      source: PluginLogSource::LogRpc,
    });
    Ok(Response::new(()))
  }

  async fn compare_contents(
    &self,
    request: Request<HostCompareContentsRequest>,
  ) -> Result<Response<CompareContentsResponse>, Status> {
    let (metadata, _, msg) = request.into_parts();
    let (chain_id, deadline_ms) = call_chain_context(&metadata);
    let entry_key = msg.entry_key;
    let inner_request = msg.request
      .ok_or_else(|| Status::invalid_argument("HostCompareContentsRequest.request is required"))?;

    if call_chain::is_expired(deadline_ms) {
      return Err(Status::deadline_exceeded(format!(
        "Call chain {} deadline has already passed", chain_id
      )));
    }
    let _guard = call_chain::push_call(&chain_id, &entry_key).map_err(Status::already_exists)?;

    let v1_request = PluginClient::convert_message(inner_request)?;
    match resolve_capability(&entry_key, CatalogueEntryType::CONTENT_MATCHER)
      .map_err(|err| Status::not_found(err.to_string()))? {
      ResolvedCapability::Core(core_key) => {
        let handler = crate::core_capabilities::lookup_core_content_matcher(&core_key)
          .ok_or_else(|| Status::not_found(format!("No core content matcher registered for '{}'", core_key)))?;
        let response = handler.compare_contents(v1_request).await
          .map_err(|err| Status::internal(format!("Core content matcher for '{}' failed: {}", core_key, err)))?;
        Ok(Response::new(PluginClient::convert_message(response)?))
      }
      ResolvedCapability::Plugin(manifest) => {
        let plugin = crate::plugin_manager::lookup_plugin(&manifest.as_dependency())
          .ok_or_else(|| Status::not_found(format!("Plugin '{}' for entry '{}' is not currently running", manifest.name, entry_key)))?;
        let response = plugin.compare_contents_with_chain(v1_request, &chain_id, deadline_ms).await
          .map_err(|err| Status::internal(format!("Call to plugin '{}' failed: {}", manifest.name, err)))?;
        Ok(Response::new(PluginClient::convert_message(response)?))
      }
    }
  }

  async fn generate_content(
    &self,
    request: Request<HostGenerateContentRequest>,
  ) -> Result<Response<GenerateContentResponse>, Status> {
    let (metadata, _, msg) = request.into_parts();
    let (chain_id, deadline_ms) = call_chain_context(&metadata);
    let entry_key = msg.entry_key;
    let inner_request = msg.request
      .ok_or_else(|| Status::invalid_argument("HostGenerateContentRequest.request is required"))?;

    if call_chain::is_expired(deadline_ms) {
      return Err(Status::deadline_exceeded(format!(
        "Call chain {} deadline has already passed", chain_id
      )));
    }
    let _guard = call_chain::push_call(&chain_id, &entry_key).map_err(Status::already_exists)?;

    let v1_request = PluginClient::convert_message(inner_request)?;
    match resolve_capability(&entry_key, CatalogueEntryType::CONTENT_GENERATOR)
      .map_err(|err| Status::not_found(err.to_string()))? {
      ResolvedCapability::Core(core_key) => {
        let handler = crate::core_capabilities::lookup_core_content_generator(&core_key)
          .ok_or_else(|| Status::not_found(format!("No core content generator registered for '{}'", core_key)))?;
        let response = handler.generate_content(v1_request).await
          .map_err(|err| Status::internal(format!("Core content generator for '{}' failed: {}", core_key, err)))?;
        Ok(Response::new(PluginClient::convert_message(response)?))
      }
      ResolvedCapability::Plugin(manifest) => {
        let plugin = crate::plugin_manager::lookup_plugin(&manifest.as_dependency())
          .ok_or_else(|| Status::not_found(format!("Plugin '{}' for entry '{}' is not currently running", manifest.name, entry_key)))?;
        let response = plugin.generate_content_with_chain(v1_request, &chain_id, deadline_ms).await
          .map_err(|err| Status::internal(format!("Call to plugin '{}' failed: {}", manifest.name, err)))?;
        Ok(Response::new(PluginClient::convert_message(response)?))
      }
    }
  }
}

/// Extract the call-chain ID and deadline from incoming callback metadata, falling back to a
/// fresh chain and the default budget if either is missing or malformed - defensive handling for
/// a plugin that didn't propagate the metadata it was given. See [`crate::call_chain`].
fn call_chain_context(metadata: &MetadataMap) -> (String, u64) {
  let chain_id = metadata.get(call_chain::CALL_CHAIN_ID_METADATA_KEY)
    .and_then(|value| value.to_str().ok())
    .map(|value| value.to_string())
    .unwrap_or_else(|| {
      warn!("Callback request had no '{}' metadata, starting a new call chain", call_chain::CALL_CHAIN_ID_METADATA_KEY);
      call_chain::new_call_chain_id()
    });
  let deadline_ms = metadata.get(call_chain::DEADLINE_METADATA_KEY)
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.parse::<u64>().ok())
    .unwrap_or_else(|| {
      warn!("Callback request had no '{}' metadata, using the default budget", call_chain::DEADLINE_METADATA_KEY);
      call_chain::default_deadline_ms()
    });
  (chain_id, deadline_ms)
}

/// Ensure the PluginHost gRPC server is running and return its port.
/// The server is started at most once per process via an `OnceLock` guard.
pub(crate) async fn ensure_plugin_host_running() -> anyhow::Result<u16> {
  if let Some(&port) = PLUGIN_HOST_PORT.get() {
    return Ok(port);
  }

  // Bind to port 0 and keep the socket open so the OS cannot reallocate the port
  // before tonic starts accepting on it.
  let std_listener = std::net::TcpListener::bind("127.0.0.1:0")
    .context("Failed to bind PluginHost server socket")?;
  let port = std_listener.local_addr()?.port();
  std_listener.set_nonblocking(true)?;

  // Only the first caller proceeds; concurrent callers return the winning port and
  // their socket is dropped (OS frees it, harmless).
  if PLUGIN_HOST_PORT.set(port).is_err() {
    return Ok(*PLUGIN_HOST_PORT.get().unwrap());
  }

  let listener = tokio::net::TcpListener::from_std(std_listener)?;
  let incoming = stream::unfold(listener, |listener| async move {
    match listener.accept().await {
      Ok((stream, _)) => Some((Ok::<_, std::io::Error>(stream), listener)),
      Err(e) => Some((Err(e), listener)),
    }
  });

  info!("Starting PluginHost gRPC server on 127.0.0.1:{}", port);

  tokio::spawn(async move {
    if let Err(err) = tonic::transport::Server::builder()
      .add_service(plugin_host_server::PluginHostServer::new(PluginHostService))
      .serve_with_incoming(incoming)
      .await
    {
      error!("PluginHost server exited with error: {}", err);
    }
  });

  Ok(port)
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;
  use std::time::Duration;

  use async_trait::async_trait;
  use tonic::Code;
  use tonic::metadata::MetadataValue;

  use crate::catalogue_manager::{CatalogueEntry, CatalogueEntryProviderType, CatalogueEntryType, register_core_entries};
  use crate::core_capabilities::{self, CoreContentGenerator, CoreContentMatcher};
  use crate::proto::{CompareContentsRequest, CompareContentsResponse, GenerateContentRequest, GenerateContentResponse};
  use crate::proto_v2;

  use super::*;

  fn core_entry(entry_type: CatalogueEntryType, key: &str) -> CatalogueEntry {
    CatalogueEntry {
      entry_type,
      provider_type: CatalogueEntryProviderType::CORE,
      plugin: None,
      key: key.to_string(),
      values: Default::default()
    }
  }

  fn request_with_metadata<T>(msg: T, chain_id: &str, deadline_ms: u64) -> Request<T> {
    let mut request = Request::new(msg);
    request.metadata_mut().insert(
      call_chain::CALL_CHAIN_ID_METADATA_KEY,
      MetadataValue::try_from(chain_id).unwrap()
    );
    request.metadata_mut().insert(
      call_chain::DEADLINE_METADATA_KEY,
      MetadataValue::try_from(deadline_ms.to_string()).unwrap()
    );
    request
  }

  struct SuccessfulCoreMatcher;

  #[async_trait]
  impl CoreContentMatcher for SuccessfulCoreMatcher {
    async fn compare_contents(&self, _request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse> {
      Ok(CompareContentsResponse::default())
    }
  }

  struct SuccessfulCoreGenerator;

  #[async_trait]
  impl CoreContentGenerator for SuccessfulCoreGenerator {
    async fn generate_content(&self, _request: GenerateContentRequest) -> anyhow::Result<GenerateContentResponse> {
      Ok(GenerateContentResponse::default())
    }
  }

  #[test_log::test(tokio::test)]
  async fn compare_contents_dispatches_to_a_registered_core_handler() {
    let key = "compare_contents_dispatches_to_a_registered_core_handler";
    register_core_entries(&vec![core_entry(CatalogueEntryType::CONTENT_MATCHER, key)]);
    core_capabilities::register_core_content_matcher(key, Arc::new(SuccessfulCoreMatcher));

    let service = PluginHostService;
    let request = request_with_metadata(
      HostCompareContentsRequest { entry_key: key.to_string(), request: Some(proto_v2::CompareContentsRequest::default()) },
      "compare_contents_dispatches_to_a_registered_core_handler-chain",
      call_chain::default_deadline_ms()
    );

    let result = plugin_host_server::PluginHost::compare_contents(&service, request).await;

    core_capabilities::deregister_core_content_matcher(key);

    assert!(result.is_ok(), "expected the call to succeed, got {:?}", result.err());
  }

  #[test_log::test(tokio::test)]
  async fn compare_contents_returns_not_found_for_an_unknown_entry_key() {
    let service = PluginHostService;
    let request = request_with_metadata(
      HostCompareContentsRequest {
        entry_key: "compare_contents_returns_not_found_for_an_unknown_entry_key".to_string(),
        request: Some(proto_v2::CompareContentsRequest::default())
      },
      "compare_contents_returns_not_found_for_an_unknown_entry_key-chain",
      call_chain::default_deadline_ms()
    );

    let result = plugin_host_server::PluginHost::compare_contents(&service, request).await;

    let status = result.expect_err("expected an error for an unregistered entry key");
    assert_eq!(status.code(), Code::NotFound);
  }

  #[test_log::test(tokio::test)]
  async fn compare_contents_rejects_an_entry_of_the_wrong_capability_shape() {
    let key = "compare_contents_rejects_an_entry_of_the_wrong_capability_shape";
    register_core_entries(&vec![core_entry(CatalogueEntryType::CONTENT_GENERATOR, key)]);

    let service = PluginHostService;
    let request = request_with_metadata(
      HostCompareContentsRequest { entry_key: key.to_string(), request: Some(proto_v2::CompareContentsRequest::default()) },
      "compare_contents_rejects_an_entry_of_the_wrong_capability_shape-chain",
      call_chain::default_deadline_ms()
    );

    let result = plugin_host_server::PluginHost::compare_contents(&service, request).await;

    let status = result.expect_err("expected an error when the entry is a generator, not a matcher");
    assert_eq!(status.code(), Code::NotFound);
  }

  #[test_log::test(tokio::test)]
  async fn compare_contents_rejects_a_cycle_within_the_same_call_chain() {
    let key = "compare_contents_rejects_a_cycle_within_the_same_call_chain";
    let chain_id = "compare_contents_rejects_a_cycle_within_the_same_call_chain-chain";
    register_core_entries(&vec![core_entry(CatalogueEntryType::CONTENT_MATCHER, key)]);
    core_capabilities::register_core_content_matcher(key, Arc::new(SuccessfulCoreMatcher));

    // Simulate this entry already being mid-flight on the chain, as if a plugin called back
    // into itself (directly, or via a longer cycle through other plugins).
    let _guard = call_chain::push_call(chain_id, key).unwrap();

    let service = PluginHostService;
    let request = request_with_metadata(
      HostCompareContentsRequest { entry_key: key.to_string(), request: Some(proto_v2::CompareContentsRequest::default()) },
      chain_id,
      call_chain::default_deadline_ms()
    );

    let result = plugin_host_server::PluginHost::compare_contents(&service, request).await;

    core_capabilities::deregister_core_content_matcher(key);

    let status = result.expect_err("expected a cycle to be rejected");
    assert_eq!(status.code(), Code::AlreadyExists);
  }

  #[test_log::test(tokio::test)]
  async fn compare_contents_rejects_a_call_chain_whose_deadline_has_passed() {
    let key = "compare_contents_rejects_a_call_chain_whose_deadline_has_passed";
    register_core_entries(&vec![core_entry(CatalogueEntryType::CONTENT_MATCHER, key)]);
    core_capabilities::register_core_content_matcher(key, Arc::new(SuccessfulCoreMatcher));

    let service = PluginHostService;
    let request = request_with_metadata(
      HostCompareContentsRequest { entry_key: key.to_string(), request: Some(proto_v2::CompareContentsRequest::default()) },
      "compare_contents_rejects_a_call_chain_whose_deadline_has_passed-chain",
      call_chain::now_ms().saturating_sub(Duration::from_secs(1).as_millis() as u64)
    );

    let result = plugin_host_server::PluginHost::compare_contents(&service, request).await;

    core_capabilities::deregister_core_content_matcher(key);

    let status = result.expect_err("expected an already-passed deadline to be rejected");
    assert_eq!(status.code(), Code::DeadlineExceeded);
  }

  #[test_log::test(tokio::test)]
  async fn generate_content_dispatches_to_a_registered_core_handler() {
    let key = "generate_content_dispatches_to_a_registered_core_handler";
    register_core_entries(&vec![core_entry(CatalogueEntryType::CONTENT_GENERATOR, key)]);
    core_capabilities::register_core_content_generator(key, Arc::new(SuccessfulCoreGenerator));

    let service = PluginHostService;
    let request = request_with_metadata(
      HostGenerateContentRequest { entry_key: key.to_string(), request: Some(proto_v2::GenerateContentRequest::default()) },
      "generate_content_dispatches_to_a_registered_core_handler-chain",
      call_chain::default_deadline_ms()
    );

    let result = plugin_host_server::PluginHost::generate_content(&service, request).await;

    core_capabilities::deregister_core_content_generator(key);

    assert!(result.is_ok(), "expected the call to succeed, got {:?}", result.err());
  }

  #[test_log::test(tokio::test)]
  async fn generate_content_returns_not_found_for_an_unknown_entry_key() {
    let service = PluginHostService;
    let request = request_with_metadata(
      HostGenerateContentRequest {
        entry_key: "generate_content_returns_not_found_for_an_unknown_entry_key".to_string(),
        request: Some(proto_v2::GenerateContentRequest::default())
      },
      "generate_content_returns_not_found_for_an_unknown_entry_key-chain",
      call_chain::default_deadline_ms()
    );

    let result = plugin_host_server::PluginHost::generate_content(&service, request).await;

    let status = result.expect_err("expected an error for an unregistered entry key");
    assert_eq!(status.code(), Code::NotFound);
  }
}
