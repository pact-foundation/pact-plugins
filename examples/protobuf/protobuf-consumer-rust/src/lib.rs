tonic::include_proto!("io.pact.plugin");

#[cfg(test)]
mod tests {
  use std::path::Path;

  use expectest::prelude::*;
  use maplit::hashmap;
  use pact_consumer::prelude::*;
  use pact_models::prelude::*;
  use prost::Message;

  use super::*;

  #[tokio::test]
  async fn test_csv_client() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut pact_builder = PactBuilder::new_v4("protobuf-consumer-rust", "protobuf-provider");
    let proto_service = pact_builder
      .using_plugin("protobuf", None).await
      .message_interaction("init plugin message", "core/interaction/message", |mut i| async move {
        let proto_file = Path::new("../../../proto/plugin.proto")
          .canonicalize().unwrap().to_string_lossy().to_string();
        i.contents_from(hashmap!{
          "proto" => proto_file,
          "message-type" => "InitPluginRequest".to_string(),
          "content-type" => "application/protobuf".to_string(),
          "implementation" => "notEmpty('pact-jvm-driver')".to_string(),
          "version" => "matching(semver, '0.0.0')".to_string()
        }).await;
        i
      })
      .await;

    for message in proto_service.messages() {
      let request = InitPluginRequest::decode(message.contents.contents.value().unwrap()).unwrap();
      expect!(request.implementation).to(be_equal_to("pact-jvm-driver"));
      expect!(request.version).to(be_equal_to("0.0.0"));
    }
  }
}
