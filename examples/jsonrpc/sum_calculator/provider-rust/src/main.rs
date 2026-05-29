use std::{path::PathBuf, time::Duration};

use anyhow::{anyhow, Context, Result};
use axum::{extract::Json, routing::post, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tracing::{debug, info, warn};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
  #[allow(dead_code)]
  jsonrpc: String,
  method: String,
  #[serde(default)]
  params: Value,
  id: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
  jsonrpc: &'static str,
  #[serde(skip_serializing_if = "Option::is_none")]
  result: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  error: Option<Value>,
  id: Value,
}

#[tokio::main]
async fn main() -> Result<()> {
  init_tracing();

  let listener = TcpListener::bind("127.0.0.1:9292")
    .await
    .context("failed to bind provider")?;
  serve(listener).await
}

async fn serve(listener: TcpListener) -> Result<()> {
  let addr = listener
    .local_addr()
    .context("failed to read provider listener address")?;
  let app = Router::new().route("/rpc", post(sum_handler));
  info!(%addr, "JSON-RPC sum provider listening");
  axum::serve(listener, app)
    .await
    .context("provider server failed")
}

async fn sum_handler(Json(request): Json<JsonRpcRequest>) -> Json<JsonRpcResponse> {
  debug!(
    jsonrpc = %request.jsonrpc,
    method = %request.method,
    id = %request.id,
    params = %request.params,
    "Received JSON-RPC request"
  );

  match sum_from_params(&request.params) {
    Ok(result) if request.method == "sum" => {
      debug!(result, "Returning successful JSON-RPC response");
      Json(JsonRpcResponse {
        jsonrpc: "2.0",
        result: Some(result),
        error: None,
        id: request.id,
      })
    }
    Ok(_) => {
      warn!(method = %request.method, "Received unsupported JSON-RPC method");
      Json(JsonRpcResponse {
        jsonrpc: "2.0",
        result: None,
        error: Some(json!({ "code": -32601, "message": "Unknown method" })),
        id: request.id,
      })
    }
    Err(error) => {
      warn!(error = %error, params = %request.params, "Received invalid JSON-RPC params");
      Json(JsonRpcResponse {
        jsonrpc: "2.0",
        result: None,
        error: Some(json!({ "code": -32602, "message": error.to_string() })),
        id: request.id,
      })
    }
  }
}

fn sum_from_params(params: &Value) -> Result<i64> {
  let values = params
    .as_array()
    .ok_or_else(|| anyhow!("params must be an array of two numbers"))?;

  if values.len() != 2 {
    return Err(anyhow!("params must contain exactly two numbers"));
  }

  let left = values[0]
    .as_i64()
    .ok_or_else(|| anyhow!("params[0] must be a number"))?;
  let right = values[1]
    .as_i64()
    .ok_or_else(|| anyhow!("params[1] must be a number"))?;

  Ok(left + right)
}

fn consumer_pact_path() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../consumer-rust/pacts/jsonrpc-consumer-rust-sum-provider.json")
}

fn init_tracing() {
  let subscriber = FmtSubscriber::builder()
    .with_env_filter(
      EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("provider_rust=debug,info")),
    )
    .pretty()
    .finish();

  if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
    eprintln!("WARN: Failed to initialise global tracing subscriber - {err}");
  }
}

#[cfg(test)]
mod tests {
  use std::process::Command;

  use super::*;

  #[test]
  fn sums_two_jsonrpc_parameters() {
    let result = sum_from_params(&json!([2, 3])).unwrap();
    assert_eq!(result, 5);
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
  #[ignore = "requires a generated pact file, a locally installed jsonrpc plugin, and a verifier build with v2 plugin support"]
  async fn verify_jsonrpc_provider() {
    init_tracing();

    let pact_path = consumer_pact_path();
    assert!(
      pact_path.exists(),
      "missing consumer pact at {}. Run `cargo test` in ../consumer-rust first.",
      pact_path.display()
    );

    let verifier = PathBuf::from(std::env::var("PACT_VERIFIER_CLI").unwrap_or_else(|_| {
      format!(
        "{}/.pact/bin/pact_verifier_cli",
        std::env::var("HOME").unwrap()
      )
    }));
    assert!(
    verifier.exists(),
    "missing pact_verifier_cli at {}. Install it with scripts/install-verifier-cli.sh or set PACT_VERIFIER_CLI.",
    verifier.display()
  );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    info!(%port, pact = %pact_path.display(), "Starting provider for verifier run");
    let server = tokio::spawn(async move {
      serve(listener).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    let output = Command::new(verifier)
      .env("pact_do_not_track", "true")
      .arg("-f")
      .arg(&pact_path)
      .arg("-p")
      .arg(port.to_string())
      .output()
      .expect("failed to invoke pact_verifier_cli");

    server.abort();
    let _ = server.await;

    assert!(
      output.status.success(),
      "provider verification failed\nstdout:\n{}\nstderr:\n{}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
  }
}
