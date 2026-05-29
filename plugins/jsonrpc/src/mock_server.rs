use std::{collections::HashSet, sync::Arc};

use anyhow::{Context, Result};
use axum::{
  body::Bytes,
  extract::{OriginalUri, State},
  http::{HeaderValue, StatusCode},
  response::IntoResponse,
  routing::any,
  Router,
};
use serde_json::json;
use tokio::{
  net::TcpListener,
  sync::{oneshot, Mutex},
  task::JoinHandle,
};

use crate::{
  jsonrpc::parse_json_body,
  pact::PactInteraction,
  proto::{ContentMismatch, MockServerResult, MockServerResults},
};

#[derive(Debug)]
pub struct RunningMockServer {
  address: String,
  port: u16,
  state: Arc<Mutex<MockServerState>>,
  shutdown: Option<oneshot::Sender<()>>,
  task: JoinHandle<()>,
}

#[derive(Debug)]
struct MockServerState {
  interactions: Vec<PactInteraction>,
  matched: HashSet<String>,
  observations: Vec<ObservedRequest>,
}

#[derive(Debug)]
struct ObservedRequest {
  path: String,
  error: String,
  mismatches: Vec<ContentMismatch>,
}

#[derive(Clone, Debug)]
struct AppState {
  state: Arc<Mutex<MockServerState>>,
}

impl RunningMockServer {
  pub async fn start(
    host_interface: &str,
    port: u32,
    interactions: Vec<PactInteraction>,
  ) -> Result<Self> {
    let bind_host = if host_interface.is_empty() {
      "127.0.0.1"
    } else {
      host_interface
    };
    let listener = TcpListener::bind((bind_host, port as u16))
      .await
      .with_context(|| format!("failed to bind JSON-RPC mock server to {bind_host}:{port}"))?;
    let local_addr = listener
      .local_addr()
      .context("failed to read JSON-RPC mock server address")?;

    let state = Arc::new(Mutex::new(MockServerState {
      interactions,
      matched: HashSet::new(),
      observations: vec![],
    }));
    let app_state = AppState {
      state: state.clone(),
    };
    let router = Router::new()
      .route("/{*path}", any(handle_request))
      .route("/", any(handle_request))
      .with_state(app_state);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let task = tokio::spawn(async move {
      let server = axum::serve(listener, router).with_graceful_shutdown(async {
        let _ = shutdown_rx.await;
      });

      if let Err(error) = server.await {
        log::error!("JSON-RPC mock server failed: {error}");
      }
    });

    Ok(Self {
      address: local_addr.ip().to_string(),
      port: local_addr.port(),
      state,
      shutdown: Some(shutdown_tx),
      task,
    })
  }

  pub fn address(&self) -> &str {
    &self.address
  }

  pub fn port(&self) -> u16 {
    self.port
  }

  pub async fn results(&self) -> MockServerResults {
    build_results(&self.state).await
  }

  pub async fn shutdown(mut self) -> Result<MockServerResults> {
    let state = self.state.clone();
    if let Some(shutdown) = self.shutdown.take() {
      let _ = shutdown.send(());
    }
    let _ = self.task.await;
    Ok(build_results(&state).await)
  }
}

async fn handle_request(
  State(app): State<AppState>,
  original_uri: OriginalUri,
  body: Bytes,
) -> impl IntoResponse {
  let path = original_uri.path().to_string();
  let body_bytes = body.to_vec();
  let body_json = match parse_json_body(&body_bytes, "mock server request body") {
    Ok(value) => value,
    Err(error) => {
      record_observation(
        &app.state,
        ObservedRequest {
          path: path.clone(),
          error: error.to_string(),
          mismatches: vec![],
        },
      )
      .await;
      return json_response(
        StatusCode::BAD_REQUEST,
        json!({ "jsonrpc": "2.0", "error": { "code": -32700, "message": error.to_string() }, "id": null }),
      );
    }
  };

  let (status, response_body, observation) = {
    let mut state = app.state.lock().await;
    match find_interaction(&mut state, &path, &body_json) {
      Ok((interaction_key, response_body)) => {
        state.matched.insert(interaction_key);
        (
          StatusCode::OK,
          response_body,
          ObservedRequest {
            path,
            error: String::new(),
            mismatches: vec![],
          },
        )
      }
      Err(error) => (
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({ "jsonrpc": "2.0", "error": { "code": -32000, "message": error.error.clone() }, "id": null }),
        ObservedRequest {
          path,
          error: error.error,
          mismatches: error.mismatches,
        },
      ),
    }
  };

  record_observation(&app.state, observation).await;
  json_response(status, response_body)
}

struct MatchFailure {
  error: String,
  mismatches: Vec<ContentMismatch>,
}

fn find_interaction(
  state: &mut MockServerState,
  path: &str,
  request_body: &serde_json::Value,
) -> std::result::Result<(String, serde_json::Value), MatchFailure> {
  let request_method = request_body
    .get("method")
    .and_then(serde_json::Value::as_str);
  let candidate = state
    .interactions
    .iter()
    .find(|interaction| {
      interaction.config.path == path
        && request_method
          .map(|method| interaction.config.request.method == method)
          .unwrap_or(true)
        && !state.matched.contains(&interaction.key)
    })
    .or_else(|| {
      state
        .interactions
        .iter()
        .find(|interaction| interaction.config.path == path)
    });

  let Some(interaction) = candidate else {
    return Err(MatchFailure {
      error: format!("No JSON-RPC interaction was configured for path '{path}'"),
      mismatches: vec![],
    });
  };

  let mismatches = interaction.config.request_mismatches(path, request_body);
  if mismatches.is_empty() {
    Ok((interaction.key.clone(), interaction.config.response_json()))
  } else {
    Err(MatchFailure {
      error: format!(
        "Request did not match interaction '{}'",
        interaction.description
      ),
      mismatches,
    })
  }
}

async fn record_observation(state: &Arc<Mutex<MockServerState>>, observation: ObservedRequest) {
  state.lock().await.observations.push(observation);
}

async fn build_results(state: &Arc<Mutex<MockServerState>>) -> MockServerResults {
  let state = state.lock().await;
  let mut results: Vec<MockServerResult> = state
    .observations
    .iter()
    .map(|observation| MockServerResult {
      path: observation.path.clone(),
      error: observation.error.clone(),
      mismatches: observation.mismatches.clone(),
    })
    .collect();

  for interaction in &state.interactions {
    if !state.matched.contains(&interaction.key) {
      results.push(MockServerResult {
        path: interaction.config.path.clone(),
        error: format!(
          "Interaction '{}' did not receive a matching request",
          interaction.description
        ),
        mismatches: vec![ContentMismatch {
          expected: None,
          actual: None,
          mismatch: "Expected JSON-RPC request was not received".to_string(),
          path: "$.request".to_string(),
          diff: String::new(),
          mismatch_type: "body".to_string(),
        }],
      });
    }
  }

  MockServerResults {
    ok: results
      .iter()
      .all(|result| result.error.is_empty() && result.mismatches.is_empty()),
    results,
  }
}

fn json_response(status: StatusCode, value: serde_json::Value) -> impl IntoResponse {
  let body = serde_json::to_vec_pretty(&value).unwrap_or_default();
  (
    status,
    [("content-type", HeaderValue::from_static("application/json"))],
    body,
  )
}
