tonic::include_proto!("area_calculator");

#[cfg(test)]
mod tests {
  use std::path::Path;

  use expectest::prelude::*;
  use pact_consumer::prelude::*;
  use serde_json::json;

  use super::*;

  #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
  async fn test_proto_client() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut pact_builder = PactBuilder::new_v4("grpc-consumer-rust", "area-calculator-provider");
    let mock_server = pact_builder
      .using_plugin("protobuf", Some("0.1.0".to_string())).await
      .synchronous_message_interaction("calculate rectangle area request", |mut i| async move {
        let proto_file = Path::new("../proto/area_calculator.proto")
          .canonicalize().unwrap().to_string_lossy().to_string();
        i.contents_from(json!({
          "pact:proto": proto_file,
          "pact:content-type": "application/protobuf",
          "pact:proto-service": "Calculator/calculate",
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
      .start_mock_server_async(Some("plugin/protobuf/transport/grpc"))
      .await;

    let url = mock_server.url();
    let mut client = calculator_client::CalculatorClient::connect(url.to_string()).await.unwrap();

    // Correct request
    let shape_message = ShapeMessage {
      shape: Some(shape_message::Shape::Rectangle(Rectangle {
        length: 4.0,
        width: 4.0
      }))
    };
    let response = client.calculate(tonic::Request::new(shape_message)).await;
    let area_message = response.unwrap();
    expect!(area_message.get_ref().value).to(be_equal_to(12.0));

    // Incorrect request, missing the length field
    // let shape_message = ShapeMessage {
    //   shape: Some(shape_message::Shape::Rectangle(Rectangle {
    //     width: 4.0, .. Rectangle::default()
    //   }))
    // };
    // client.calculate(tonic::Request::new(shape_message)).await;
  }
}
