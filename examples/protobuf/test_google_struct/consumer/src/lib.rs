tonic::include_proto!("google_structs");

#[cfg(test)]
mod tests {
  use std::path::Path;

  use expectest::prelude::*;
  use pact_consumer::prelude::*;
  use prost::Message;
  use serde_json::json;

  use super::*;

  #[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
  async fn test_message_with_google_struct() {
    let mut pact_builder = &mut PactBuilderAsync::new_v4("google_structs-consumer", "google_structs-provider");
    pact_builder = pact_builder.using_plugin("protobuf", None).await;
    let proto_service = pact_builder
      .message_interaction("request for something with a google struct", |mut i| async move {
        let project_dir = Path::new(option_env!("CARGO_MANIFEST_DIR").unwrap());
        let proto_file = project_dir.join("..").join("proto").join("google_structs.proto");

        i.contents_from(json!({
          "pact:proto": proto_file.to_str().unwrap(),
          "pact:content-type": "application/protobuf",
          "pact:message-type": "Request",
          "params": {
            "kind": "general",
            "message": "test",
            "xids": [1, 3, 6, 79.0],
            "other": { "a": true, "b": false }
          },
          "name": "matching(type, 'Some Name')"
        })).await;
        i.test_name("test_google_struct::test::test_message_with_google_struct");
        i
      })
      .await;

    let _pact = proto_service.build();
    for message in proto_service.messages() {
      let bytes = message.contents.contents.value().unwrap();
      let request = Request::decode(bytes).unwrap();

      expect!(request.name).to(be_equal_to("Some Name"));
      // expect!(request.params.unwrap()).to(be_equal_to(Struct {
      //   fields: btreemap!{
      //     "kind".to_string() => Value {
      //       kind: Some(Kind::StringValue("general".to_string()))
      //     },
      //     "message".to_string() => Value {
      //       kind: Some(Kind::StringValue("test".to_string()))
      //     }
      //   },
      // }));
    }
  }
}
