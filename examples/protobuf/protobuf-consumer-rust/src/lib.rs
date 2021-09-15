tonic::include_proto!("io.pact.plugin");

#[cfg(test)]
mod tests {
  use std::path::Path;

  use expectest::prelude::*;
  use pact_consumer::prelude::*;
  use pact_models::prelude::*;
  use prost::Message;
  use serde_json::{json, Map};

  use super::*;

  #[tokio::test]
  async fn test_proto_client() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut pact_builder = PactBuilder::new_v4("protobuf-consumer-rust", "protobuf-provider");
    let proto_service = pact_builder
      .using_plugin("protobuf", None).await
      .message_interaction("init plugin message", "core/interaction/message", |mut i| async move {
        let proto_file = Path::new("../../../proto/plugin.proto")
          .canonicalize().unwrap().to_string_lossy().to_string();
        i.contents_from(json!({
          "proto": proto_file,
          "message-type": "InitPluginRequest",
          "content-type": "application/protobuf",
          "implementation": "notEmpty('pact-jvm-driver')",
          "version": "matching(semver, '0.0.0')"
        })).await;
        i
      })
      .await;

    for message in proto_service.messages() {
      let request = InitPluginRequest::decode(message.contents.contents.value().unwrap()).unwrap();
      expect!(request.implementation).to(be_equal_to("pact-jvm-driver"));
      expect!(request.version).to(be_equal_to("0.0.0"));
    }
  }

  #[tokio::test]
  async fn proto_with_message_fields() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut pact_builder = PactBuilder::new_v4("protobuf-consumer-rust", "protobuf-provider");
    let proto_service = pact_builder
      .using_plugin("protobuf", None).await
      .message_interaction("ConfigureInteractionResponse message", "core/interaction/message", |mut i| async move {
        let proto_file = Path::new("../../../proto/plugin.proto")
          .canonicalize().unwrap().to_string_lossy().to_string();

        i.contents_from(json!({
          "proto": proto_file,
          "message-type": "ConfigureInteractionResponse",
          "content-type": "application/protobuf",
          "contents": {
            "contentType": "notEmpty('application/json')",
            "content": "matching(contentType, 'application/json', '{}')",
            "contentTypeHint": "matching(equalTo, 'TEXT')"
          }
        })).await;
        i
      })
      .await;

    for message in proto_service.messages() {
      let request = ConfigureInteractionResponse::decode(message.contents.contents.value().unwrap()).unwrap();
      let contents = request.contents.unwrap();
      let content = contents.clone().content.unwrap();
      expect!(&contents.content_type).to(be_equal_to("application/json"));
      expect!(content).to(be_equal_to("{}".bytes().collect::<Vec<u8>>()));
      expect!(contents.content_type_hint()).to(be_equal_to(body::ContentTypeHint::Text));
    }
  }
}
