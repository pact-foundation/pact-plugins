use std::path::Path;

use anyhow::anyhow;
use async_trait::async_trait;
use expectest::prelude::*;
use pact_consumer::prelude::*;
use prost::Message;
use serde_json::json;

use pact_plugin_driver::plugin_manager::init_handshake;
use pact_plugin_driver::plugin_models::{PactPluginManifest, PactPluginRpc};
use pact_plugin_driver::proto::*;

struct MockPlugin {
  pub request: InitPluginRequest,
  pub response: InitPluginResponse
}

#[async_trait]
impl PactPluginRpc for MockPlugin {
  async fn init_plugin(&self, request: InitPluginRequest) -> anyhow::Result<InitPluginResponse> {
    if self.request.implementation == request.implementation {
      Ok(self.response.clone())
    } else {
      Err(anyhow!("Received incorrect request, expected {:?} but got {:?}", self.request, request))
    }
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

  async fn prepare_interaction_for_verification(&self, _request: VerificationPreparationRequest) -> anyhow::Result<VerificationPreparationResponse> {
    unimplemented!()
  }

  async fn verify_interaction(&self, _request: VerifyInteractionRequest) -> anyhow::Result<VerifyInteractionResponse> {
    unimplemented!()
  }

  async fn update_catalogue(&self, _request: Catalogue) -> anyhow::Result<()> {
    unimplemented!()
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_proto_client() {
  let _ = env_logger::builder().is_test(true).try_init();

  let mut pact_builder = PactBuilder::new_v4("pact-rust-driver", "plugin");
  let proto_service = pact_builder
    .using_plugin("protobuf", None).await
    .synchronous_message_interaction("init plugin request", |mut i| async move {
      let project_dir = Path::new(option_env!("CARGO_MANIFEST_DIR").unwrap());
      println!("project_dir = {:?}", project_dir);
      let proto_file = project_dir.join("..").join("driver").join("plugin.proto");
      println!("proto_file = {:?}", proto_file);

      i.contents_from(json!({
          "pact:proto": proto_file.to_str().unwrap(),
          "pact:content-type": "application/protobuf",
          "pact:proto-service": "PactPlugin/InitPlugin",
          "request": {
            "implementation": "notEmpty('plugin-driver-rust')",
            "version": "matching(semver, '0.0.0')"
          },
          "response": {
            "catalogue": {
              "pact:match" : "eachValue(matching($'CatalogueEntry'))",
              "CatalogueEntry": {
                "type": "matching(regex, 'CONTENT_MATCHER|CONTENT_GENERATOR', 'CONTENT_MATCHER')",
                "key": "notEmpty('test')"
              }
            }
          }
        })).await;
      i.test_name("pact::test_proto_client");
      i
    })
    .await;

  for message in proto_service.synchronous_messages() {
    let bytes = message.request.contents.value().unwrap();
    let request = InitPluginRequest::decode(bytes).unwrap();
    let bytes = message.response.first().unwrap().contents.value().unwrap();
    let response = InitPluginResponse::decode(bytes).unwrap();
    let manifest = PactPluginManifest {
      name: "Test".to_string(),
      .. PactPluginManifest::default()
    };
    let mock_plugin = MockPlugin { request, response };

    let result = init_handshake(&manifest, &mock_plugin).await;

    expect!(result).to(be_ok());
  }
}
