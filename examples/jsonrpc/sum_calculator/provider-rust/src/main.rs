use std::{path::PathBuf, time::Duration};

use anyhow::{anyhow, Context, Result};
use axum::{extract::Json, routing::post, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpListener;

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
  let listener = TcpListener::bind("127.0.0.1:9292")
    .await
    .context("failed to bind provider")?;
  serve(listener).await
}

async fn serve(listener: TcpListener) -> Result<()> {
  let app = Router::new().route("/rpc", post(sum_handler));
  axum::serve(listener, app)
    .await
    .context("provider server failed")
}

async fn sum_handler(Json(request): Json<JsonRpcRequest>) -> Json<JsonRpcResponse> {
  match sum_from_params(&request.params) {
    Ok(result) if request.method == "sum" => Json(JsonRpcResponse {
      jsonrpc: "2.0",
      result: Some(result),
      error: None,
      id: request.id,
    }),
    Ok(_) => Json(JsonRpcResponse {
      jsonrpc: "2.0",
      result: None,
      error: Some(json!({ "code": -32601, "message": "Unknown method" })),
      id: request.id,
    }),
    Err(error) => Json(JsonRpcResponse {
      jsonrpc: "2.0",
      result: None,
      error: Some(json!({ "code": -32602, "message": error.to_string() })),
      id: request.id,
    }),
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
    let _ = env_logger::builder().is_test(true).try_init();

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
