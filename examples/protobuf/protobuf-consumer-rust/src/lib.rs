tonic::include_proto!("io.pact.plugin");

#[cfg(test)]
mod tests {
  use std::path::Path;
  use std::collections::HashSet;

  use expectest::prelude::*;
  use maplit::hashset;
  use pact_consumer::prelude::*;
  use prost::Message;
  use serde_json::json;

  use pact_plugin_driver::utils::proto_value_to_string;

  use super::*;

  #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
  async fn test_proto_client() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut pact_builder = PactBuilderAsync::new_v4("protobuf-consumer-rust", "protobuf-provider");
    let proto_service = pact_builder
      .using_plugin("protobuf", None).await
      .message_interaction("init plugin message", |mut i| async move {
        let proto_file = Path::new("../../../proto/plugin.proto")
          .canonicalize().unwrap().to_string_lossy().to_string();
        i.contents_from(json!({
          "pact:proto": proto_file,
          "pact:message-type": "InitPluginRequest",
          "pact:content-type": "application/protobuf",
          "implementation": "notEmpty('pact-jvm-driver')",
          "version": "matching(semver, '0.0.0')"
        })).await;
        i
      })
      .await;

    for message in proto_service.messages() {
      let bytes = message.contents.contents.value().unwrap();
      let request = InitPluginRequest::decode(bytes).unwrap();
      expect!(request.implementation).to(be_equal_to("pact-jvm-driver"));
      expect!(request.version).to(be_equal_to("0.0.0"));
    }
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
  async fn proto_with_message_fields() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut pact_builder = PactBuilderAsync::new_v4("protobuf-consumer-rust", "protobuf-provider");
    let proto_service = pact_builder
      .using_plugin("protobuf", None).await
      .message_interaction("Configure Interaction Response", |mut i| async move {
        let proto_file = Path::new("../../../proto/plugin.proto")
          .canonicalize().unwrap().to_string_lossy().to_string();

        i.contents_from(json!({
          "pact:proto": proto_file,
          "pact:message-type": "InteractionResponse",
          "pact:content-type": "application/protobuf",
          "contents": {
            "contentType": "notEmpty('application/json')",
            "content": "matching(contentType, 'application/json', '{}')",
            "contentTypeHint": "matching(equalTo, 'TEXT')"
          },
          "rules": {
            "pact:match": "eachKey(matching(regex, '\\$(\\.\\w+)+', '$.test.one')), eachValue(matching(type, null))",
            "$.test.one": {
              "rule": {
                "pact:match": "eachValue(matching($'items'))",
                "items": {
                  "type": "notEmpty('regex')"
                }
              }
            }
          },
          "generators": {
            "$.test.one": {
              "type": "notEmpty('DateTime')",
              "values": {
                "format": "matching(equalTo, 'YYYY-MM-DD')"
              }
            },
            "$.test.two": {
              "type": "notEmpty('DateTime')",
              "values": {
                "format": "matching(equalTo, 'YYYY-MM-DD')"
              }
            }
          }
        })).await;

        i
      })
      .await;

    for message in proto_service.messages() {
      let response = InteractionResponse::decode(message.contents.contents.value().unwrap()).unwrap();
      let contents = response.contents.unwrap();
      let content = contents.clone().content.unwrap();
      expect!(&contents.content_type).to(be_equal_to("application/json"));
      expect!(content).to(be_equal_to("{}".bytes().collect::<Vec<u8>>()));
      expect!(contents.content_type_hint()).to(be_equal_to(body::ContentTypeHint::Text));

      expect!(response.generators.len()).to(be_equal_to(2));
      expect!(response.generators.keys().map(|k| k.as_str()).collect::<HashSet<&str>>()).to( be_equal_to(hashset!["$.test.one", "$.test.two"]));
      expect!(response.generators.get("$.test.one").map(|v| v.r#type.as_str())).to( be_some().value("DateTime"));
      expect!(response.generators.get("$.test.one").map(|v| v.values.as_ref().map(|vals| vals.fields.get("format").map(|f| proto_value_to_string(f))))
        .flatten().flatten().flatten()).to( be_some().value("YYYY-MM-DD"));

      expect!(response.rules.len()).to(be_equal_to(1));
      expect!(response.rules.keys().collect::<Vec<&String>>()).to(be_equal_to(vec!["$.test.one"]));
      let matching_rules = response.rules.get("$.test.one").unwrap();
      expect!(matching_rules.rule.len()).to(be_equal_to(1));
      expect!(matching_rules.rule.get(0).map(|rule| rule.r#type.as_str())).to(be_some().value("regex"));
    }
  }
}
