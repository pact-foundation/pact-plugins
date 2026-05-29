use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct JsonRpcClient {
  base_url: String,
  http_client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct SumRequest {
  jsonrpc: &'static str,
  method: &'static str,
  params: [i32; 2],
  id: u64,
}

#[derive(Debug, Deserialize)]
struct SumResponse {
  #[allow(dead_code)]
  jsonrpc: String,
  result: Option<i32>,
  error: Option<serde_json::Value>,
  #[allow(dead_code)]
  id: u64,
}

impl JsonRpcClient {
  pub fn new(base_url: impl Into<String>) -> Self {
    Self {
      base_url: base_url.into().trim_end_matches('/').to_string(),
      http_client: reqwest::Client::new(),
    }
  }

  pub async fn sum(&self, left: i32, right: i32) -> Result<i32> {
    let response = self
      .http_client
      .post(format!("{}/rpc", self.base_url))
      .json(&SumRequest {
        jsonrpc: "2.0",
        method: "sum",
        params: [left, right],
        id: 1,
      })
      .send()
      .await
      .context("failed to call JSON-RPC provider")?;

    let response = response
      .error_for_status()
      .context("provider returned an error HTTP status")?;
    let body: SumResponse = response
      .json()
      .await
      .context("failed to decode JSON-RPC response")?;

    if let Some(error) = body.error {
      Err(anyhow!("provider returned a JSON-RPC error: {error}"))
    } else {
      body
        .result
        .context("provider did not return a JSON-RPC result")
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{fs, path::PathBuf};

  use expectest::prelude::*;
  use pact_consumer::{mock_server::StartMockServerAsync, prelude::*};
  use serde_json::json;

  use super::JsonRpcClient;

  #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
  async fn jsonrpc_transport_plugin_starts_a_mock_server() {
    let _ = env_logger::builder().is_test(true).try_init();

    let pact_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("pacts");
    fs::create_dir_all(&pact_dir).unwrap();
    std::env::set_var("PACT_OUTPUT_DIR", &pact_dir);

    let mut pact_builder = PactBuilderAsync::new_v4("jsonrpc-consumer-rust", "sum-provider");
    let mock_server = pact_builder
      .using_plugin("jsonrpc", None)
      .await
      .synchronous_message_interaction("sum numbers request", |mut interaction| async move {
        interaction
          .contents_from(json!({
            "pact:content-type": "application/json-rpc",
            "path": "/rpc",
            "request": {
              "method": "sum",
              "params": [2, 3],
              "id": 1
            },
            "response": {
              "result": 5
            }
          }))
          .await;
        interaction
      })
      .await
      .start_mock_server_async(Some("jsonrpc/transport/jsonrpc"), None)
      .await;

    let client = JsonRpcClient::new(mock_server.url().to_string());
    let result = client.sum(2, 3).await.unwrap();

    expect!(result).to(be_equal_to(5));
  }
}
