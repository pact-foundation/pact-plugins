mod jsonrpc;
mod mock_server;
mod pact;
mod proto;

use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Context};
use log::{debug, trace};
use tokio::{net::TcpListener, sync::Mutex};
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;

use crate::{
  jsonrpc::{config_from_struct, override_string, parse_json_body},
  mock_server::RunningMockServer,
  pact::PactInteraction,
  proto::{
    body::ContentTypeHint, catalogue_entry::EntryType,
    init_plugin_response::Response as InitResponse, pact_plugin_server::PactPluginServer,
    verification_preparation_response::Response as PreparationResponse,
    verify_interaction_response::Response as VerifyResponse, Body, Catalogue, CatalogueEntry,
    ConfigureInteractionRequest, ConfigureInteractionResponse, GenerateContentRequest,
    GenerateContentResponse, InitPluginRequest, InitPluginResponse, InitPluginSuccess,
    InteractionData, InteractionResponse, MockServerDetails, MockServerRequest,
    MockServerResults, PluginConfiguration, StartMockServerRequest, StartMockServerResponse,
    VerificationPreparationRequest, VerificationPreparationResponse, VerificationResult,
    VerificationResultItem, VerifyInteractionRequest, VerifyInteractionResponse,
  },
};

use proto::pact_plugin_server::PactPlugin;

const PLUGIN_NAME: &str = "jsonrpc";

#[derive(Debug, Default)]
struct JsonRpcPlugin {
  mock_servers: Arc<Mutex<HashMap<String, RunningMockServer>>>,
}

#[tonic::async_trait]
impl PactPlugin for JsonRpcPlugin {
  async fn init_plugin(
    &self,
    request: Request<InitPluginRequest>,
  ) -> Result<Response<InitPluginResponse>, Status> {
    debug!(
      "Init request from {}/{} with host capabilities {:?}",
      request.get_ref().implementation,
      request.get_ref().version,
      request.get_ref().host_capabilities
    );

    Ok(Response::new(InitPluginResponse {
      response: Some(InitResponse::Success(InitPluginSuccess {
        catalogue: vec![
          CatalogueEntry {
            r#type: EntryType::ContentMatcher as i32,
            key: PLUGIN_NAME.to_string(),
            values: HashMap::from([(
              "content-types".to_string(),
              "application/json-rpc".to_string(),
            )]),
          },
          CatalogueEntry {
            r#type: EntryType::ContentGenerator as i32,
            key: PLUGIN_NAME.to_string(),
            values: HashMap::from([(
              "content-types".to_string(),
              "application/json-rpc".to_string(),
            )]),
          },
          CatalogueEntry {
            r#type: EntryType::Transport as i32,
            key: PLUGIN_NAME.to_string(),
            values: HashMap::new(),
          },
        ],
        plugin_capabilities: vec![],
      })),
    }))
  }

  async fn update_catalogue(&self, _request: Request<Catalogue>) -> Result<Response<()>, Status> {
    Ok(Response::new(()))
  }

  async fn compare_contents(
    &self,
    _request: Request<proto::CompareContentsRequest>,
  ) -> Result<Response<proto::CompareContentsResponse>, Status> {
    Err(Status::unimplemented(
      "jsonrpc is a transport plugin and does not compare standalone contents",
    ))
  }

  async fn configure_interaction(
    &self,
    request: Request<ConfigureInteractionRequest>,
  ) -> Result<Response<ConfigureInteractionResponse>, Status> {
    let request = request.get_ref();
    if !request.content_type.is_empty()
      && request.content_type != "application/json"
      && request.content_type != "application/json-rpc"
    {
      return Err(Status::invalid_argument(format!(
        "JSON-RPC interactions only support application/json-rpc configuration content, got '{}'",
        request.content_type
      )));
    }

    let interaction_config = request.contents_config.as_ref();
    trace!("Got interaction configuration: {:?}", interaction_config);
    let Some(config) = config_from_struct(interaction_config)
      .map_err(|error| {
        Status::invalid_argument(format!("Failed to resolve interaction configuration: {:#}", error))
      })?
    else {
      return Err(Status::invalid_argument(
        "missing JSON-RPC interaction configuration",
      ));
    };

    let request_body = config
      .request_body()
      .map_err(|error| Status::aborted(error.to_string()))?;
    let response_body = config
      .response_body()
      .map_err(|error| Status::aborted(error.to_string()))?;
    let plugin_configuration = PluginConfiguration {
      interaction_configuration: Some(proto::json_to_proto_struct(
        &serde_json::to_value(&config).map_err(|error| Status::aborted(error.to_string()))?,
      )),
      pact_configuration: None,
    };

    Ok(Response::new(ConfigureInteractionResponse {
    error: String::new(),
    interaction: vec![
      InteractionResponse {
        contents: Some(Body {
          content_type: "application/json".to_string(),
          content: Some(request_body),
          content_type_hint: ContentTypeHint::Text as i32,
        }),
        message_metadata: Some(proto::json_to_proto_struct(&serde_json::json!({
          "path": config.path,
          "method": "POST"
        }))),
        plugin_configuration: Some(plugin_configuration.clone()),
        interaction_markup: format!(
          "### JSON-RPC request\n\n`POST {}`\n\n```json\n{}\n```\n\n### JSON-RPC response\n\n```json\n{}\n```",
          config.path,
          serde_json::to_string_pretty(&config.request_json()).unwrap_or_default(),
          serde_json::to_string_pretty(&config.response_json()).unwrap_or_default()
        ),
        interaction_markup_type: proto::interaction_response::MarkupType::CommonMark as i32,
        part_name: "request".to_string(),
        ..InteractionResponse::default()
      },
      InteractionResponse {
        contents: Some(Body {
          content_type: "application/json".to_string(),
          content: Some(response_body),
          content_type_hint: ContentTypeHint::Text as i32,
        }),
        plugin_configuration: Some(plugin_configuration),
        part_name: "response".to_string(),
        ..InteractionResponse::default()
      },
    ],
    plugin_configuration: None,
  }))
  }

  async fn generate_content(
    &self,
    _request: Request<GenerateContentRequest>,
  ) -> Result<Response<GenerateContentResponse>, Status> {
    Err(Status::unimplemented(
      "jsonrpc does not currently generate alternate content variants",
    ))
  }

  async fn start_mock_server(
    &self,
    request: Request<StartMockServerRequest>,
  ) -> Result<Response<StartMockServerResponse>, Status> {
    let request = request.get_ref();
    if request.interactions.is_empty() {
      return Ok(Response::new(StartMockServerResponse {
        response: Some(proto::start_mock_server_response::Response::Error(
          "The request did not contain any JSON-RPC plugin interactions".to_string(),
        )),
      }));
    }

    let interactions: Vec<PactInteraction> = request.interactions.iter()
      .enumerate()
      .map(|(index, contents)| {
        let config_value = contents.plugin_configuration.as_ref()
          .and_then(|pc| pc.interaction_configuration.as_ref())
          .map(proto::proto_struct_to_json)
          .unwrap_or_else(|| serde_json::json!({}));
        let config = crate::jsonrpc::JsonRpcInteractionConfig::from_contents_config(config_value)
          .map_err(|error| Status::invalid_argument(format!("interaction {index}: {error}")))?;
        Ok(PactInteraction {
          key: format!("interaction-{index}"),
          description: contents.interaction_type.clone(),
          config,
        })
      })
      .collect::<Result<Vec<_>, Status>>()?;

    let server = RunningMockServer::start(&request.host_interface, request.port, interactions)
      .await
      .map_err(|error| Status::aborted(error.to_string()))?;
    let key = Uuid::new_v4().to_string();
    let details = MockServerDetails {
      key: key.clone(),
      port: server.port() as u32,
      address: format!("http://{}:{}", server.address(), server.port()),
    };
    self.mock_servers.lock().await.insert(key, server);

    Ok(Response::new(StartMockServerResponse {
      response: Some(proto::start_mock_server_response::Response::Details(
        details,
      )),
    }))
  }

  async fn shutdown_mock_server(
    &self,
    request: Request<MockServerRequest>,
  ) -> Result<Response<MockServerResults>, Status> {
    let server_key = request.into_inner().server_key;
    let Some(server) = self.mock_servers.lock().await.remove(&server_key) else {
      return Err(Status::not_found(format!(
        "mock server '{server_key}' was not found"
      )));
    };

    let results = server
      .shutdown()
      .await
      .map_err(|error| Status::aborted(error.to_string()))?;
    Ok(Response::new(MockServerResults {
      ok: results.ok,
      results: results.results,
    }))
  }

  async fn get_mock_server_results(
    &self,
    request: Request<MockServerRequest>,
  ) -> Result<Response<proto::MockServerResults>, Status> {
    let server_key = request.into_inner().server_key;
    let servers = self.mock_servers.lock().await;
    let Some(server) = servers.get(&server_key) else {
      return Err(Status::not_found(format!(
        "mock server '{server_key}' was not found"
      )));
    };

    Ok(Response::new(server.results().await))
  }

  async fn prepare_interaction_for_verification(
    &self,
    request: Request<VerificationPreparationRequest>,
  ) -> Result<Response<VerificationPreparationResponse>, Status> {
    let request = request.get_ref();
    let config_value = request
      .interaction_contents
      .as_ref()
      .and_then(|ic| ic.plugin_configuration.as_ref())
      .and_then(|pc| pc.interaction_configuration.as_ref())
      .map(proto::proto_struct_to_json)
      .unwrap_or_else(|| serde_json::json!({}));
    let config = crate::jsonrpc::JsonRpcInteractionConfig::from_contents_config(config_value)
      .map_err(|error| Status::aborted(error.to_string()))?;
    let request_body = config
      .request_body()
      .map_err(|error| Status::aborted(error.to_string()))?;

    Ok(Response::new(VerificationPreparationResponse {
      response: Some(PreparationResponse::InteractionData(InteractionData {
        body: Some(proto::json_body(request_body)),
        metadata: config.interaction_metadata().into_iter().collect(),
      })),
    }))
  }

  async fn verify_interaction(
    &self,
    request: Request<VerifyInteractionRequest>,
  ) -> Result<Response<VerifyInteractionResponse>, Status> {
    let request = request.get_ref();
    let config_value = request
      .interaction_contents
      .as_ref()
      .and_then(|ic| ic.plugin_configuration.as_ref())
      .and_then(|pc| pc.interaction_configuration.as_ref())
      .map(proto::proto_struct_to_json)
      .unwrap_or_else(|| serde_json::json!({}));
    let interaction = crate::jsonrpc::JsonRpcInteractionConfig::from_contents_config(config_value)
      .map_err(|error| Status::aborted(error.to_string()))?;
    let Some(interaction_data) = &request.interaction_data else {
      return Ok(Response::new(VerifyInteractionResponse {
        response: Some(VerifyResponse::Error(
          "interactionData is required".to_string(),
        )),
      }));
    };

    let config = request
      .config
      .as_ref()
      .map(proto::proto_struct_to_json)
      .unwrap_or_else(|| serde_json::json!({}));
    let host = config
      .get("host")
      .and_then(serde_json::Value::as_str)
      .unwrap_or("127.0.0.1");
    let port = config
      .get("port")
      .and_then(serde_json::Value::as_u64)
      .unwrap_or(0) as u32;
    if port == 0 {
      return Ok(Response::new(VerifyInteractionResponse {
        response: Some(VerifyResponse::Error(
          "provider config must include a non-zero 'port'".to_string(),
        )),
      }));
    }
    let scheme = config
      .get("scheme")
      .and_then(serde_json::Value::as_str)
      .unwrap_or("http");
    let override_path = override_string(&interaction_data.metadata, "request.path")
      .map_err(|error| Status::invalid_argument(error.to_string()))?;
    let url = interaction.provider_url(scheme, host, port, override_path.as_deref());
    let body = interaction_data
      .body
      .as_ref()
      .and_then(|body| body.content.clone())
      .ok_or_else(|| Status::invalid_argument("interactionData.body.content is required"))?;
    let client = reqwest::Client::new();
    let response = client
      .post(&url)
      .header("content-type", "application/json")
      .body(body.clone())
      .send()
      .await
      .map_err(|error| Status::aborted(format!("failed to call provider at {url}: {error}")))?;
    let status_code = response.status();
    let bytes = response
      .bytes()
      .await
      .map_err(|error| Status::aborted(error.to_string()))?;
    let actual_json = match parse_json_body(&bytes, "provider response body") {
      Ok(value) => value,
      Err(error) => {
        return Ok(Response::new(VerifyInteractionResponse {
          response: Some(VerifyResponse::Result(VerificationResult {
            success: false,
            response_data: Some(InteractionData {
              body: Some(proto::json_body(bytes.to_vec())),
              metadata: HashMap::new(),
            }),
            mismatches: vec![VerificationResultItem {
              result: Some(proto::verification_result_item::Result::Error(
                error.to_string(),
              )),
            }],
            output: vec![
              format!("POST {url}"),
              format!("Received HTTP {}", status_code.as_u16()),
            ],
          })),
        }));
      }
    };

    let mut mismatches = vec![];
    if !status_code.is_success() {
      mismatches.push(VerificationResultItem {
        result: Some(proto::verification_result_item::Result::Error(format!(
          "Expected a successful HTTP response but received {}",
          status_code.as_u16()
        ))),
      });
    }

    mismatches.extend(
      interaction
        .response_mismatches(&actual_json)
        .into_iter()
        .map(|mismatch| VerificationResultItem {
          result: Some(proto::verification_result_item::Result::Mismatch(mismatch)),
        }),
    );

    Ok(Response::new(VerifyInteractionResponse {
      response: Some(VerifyResponse::Result(VerificationResult {
        success: mismatches.is_empty(),
        response_data: Some(InteractionData {
          body: Some(proto::json_body(bytes.to_vec())),
          metadata: HashMap::new(),
        }),
        mismatches,
        output: vec![
          format!("POST {url}"),
          format!("Received HTTP {}", status_code.as_u16()),
        ],
      })),
    }))
  }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  env_logger::init();

  let listener = TcpListener::bind("127.0.0.1:0")
    .await
    .context("failed to bind JSON-RPC plugin gRPC server")?;
  let address = listener
    .local_addr()
    .context("failed to read plugin server address")?;
  println!(
    "{}",
    serde_json::json!({ "port": address.port(), "serverKey": Uuid::new_v4().to_string() })
  );

  let stream = tokio_stream::wrappers::TcpListenerStream::new(listener);
  let plugin = JsonRpcPlugin::default();
  Server::builder()
    .add_service(PactPluginServer::new(plugin))
    .serve_with_incoming(stream)
    .await
    .map_err(|error| anyhow!("JSON-RPC plugin server failed: {error}"))
}
