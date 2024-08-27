tonic::include_proto!("area_calculator");

#[cfg(test)]
mod tests {
  use std::path::Path;
  use bytes::BytesMut;
  use expectest::prelude::*;
  use pact_consumer::prelude::*;
  use pact_consumer::mock_server::StartMockServerAsync;
  use prost::Message;
  use reqwest::Client;
  use serde_json::json;

  use super::*;

  #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
  async fn test_area_calculator_client() {
    let mut pact_builder = PactBuilderAsync::new_v4("area-calculator-consumer", "area-calculator-provider");
    let mock_server = pact_builder
      .using_plugin("protobuf", None).await
      .synchronous_message_interaction("calculate rectangle area request", |mut i| async move {
        let proto_file = Path::new("../proto/area_calculator.proto")
          .canonicalize().unwrap().to_string_lossy().to_string();
        i.contents_from(json!({
          "pact:proto": proto_file,
          "pact:content-type": "application/protobuf",
          "pact:proto-service": "Calculator/calculateOne",

          "request": {
            "rectangle": {
              "length": "matching(number, 3)",
              "width": "matching(number, 4)"
            }
          },

          "response": {
            "value": "matching(number, 12)"
          }
        })).await;
        i
      })
      .await
      .start_mock_server_async(Some("protobuf/transport/grpc"), None)
      .await;

    let url = mock_server.url();
    let mut client = calculator_client::CalculatorClient::connect(url.to_string()).await.unwrap();

    let shape_message = ShapeMessage {
      shape: Some(shape_message::Shape::Rectangle(Rectangle {
        length: 4.0,
        width: 4.0
      }))
    };
    let response = client.calculate_one(tonic::Request::new(shape_message)).await;
    let area_message = response.unwrap();
    expect!(area_message.get_ref().value.get(0).unwrap()).to(be_equal_to(&12.0));
  }

  #[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 1))]
  async fn test_area_calculator_client_via_http() {
    let project_dir = Path::new(option_env!("CARGO_MANIFEST_DIR").unwrap());
    let proto_file = project_dir.join("..").join("proto").join("area_calculator.proto");

    let mut pact_builder = PactBuilderAsync::new_v4("area-calculator-consumer", "area-calculator-provider");
    pact_builder
      .using_plugin("protobuf", None).await
      .interaction("calculate rectangle area request via HTTP", "", |mut i| async {
        i.request
          .post()
          .path("/Calculator/calculateOne")
          .contents("application/protobuf".into(), json!({
            "pact:proto": proto_file.to_str().unwrap(),
            "pact:content-type": "application/protobuf;message=.area_calculator.ShapeMessage",
            "pact:message-type": "ShapeMessage",
            "rectangle": {
              "length": "matching(number, 3)",
              "width": "matching(number, 4)"
            }
          }))
          .await;
        i.response
          .contents("application/protobuf".into(), json!({
            "pact:proto": proto_file.to_str().unwrap(),
            "pact:content-type": "application/protobuf;message=.area_calculator.AreaResponse",
            "pact:message-type": "AreaResponse",
            "value": "matching(number, 12)"
          })).await;

        i
      })
      .await;

    let mock_service = pact_builder
      .start_mock_server_async(None, None).await;
    let mock_url = mock_service.url();

    let shape = ShapeMessage {
      shape: Some(shape_message::Shape::Rectangle(Rectangle { length: 5.0, width: 10.0 })),
      .. ShapeMessage::default()
    };
    let mut buffer = BytesMut::new();
    shape.encode(&mut buffer).unwrap();
    let response = Client::new()
      .post(format!("{}Calculator/calculateOne", mock_url))
      .header("content-type", "application/protobuf;message=.area_calculator.ShapeMessage")
      .body(buffer.freeze())
      .send()
      .await
      .unwrap()
      .bytes()
      .await
      .unwrap();
    let area = AreaResponse::decode(response).unwrap();
    expect!(area.value[0]).to(be_equal_to(12.0));
  }
}
