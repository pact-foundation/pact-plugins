//! PluginHost gRPC server — receives Log RPCs from running plugins

use std::sync::OnceLock;

use anyhow::Context;
use futures_util::stream;
use tonic::{Request, Response, Status};
use tracing::{error, info};

use crate::plugin_log_sink::{PluginLogEntry, PluginLogSource, emit_plugin_log};
use crate::proto_v2::{LogMessage, plugin_host_server};

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
