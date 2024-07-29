tonic::include_proto!("area_calculator");

#[cfg(test)]
mod tests {
  use std::path::Path;

  use expectest::prelude::*;
  use pact_consumer::prelude::*;
  use pact_consumer::mock_server::StartMockServerAsync;
  use serde_json::json;

  use super::*;

  /// Main test method for the AreaCalculator calculate service method call.
  #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
  async fn test_proto_client() {
    // Configures the Pact interaction for the test. This will load the Protobuf plugin, which will provide all the
    // Protobuf and gRPC support to the Pact framework.
    let mut pact_builder = PactBuilderAsync::new_v4("grpc-consumer-rust", "area-calculator-provider");
    let mock_server = pact_builder
      // Tell Pact we need the Protobuf plugin
      .using_plugin("protobuf", None).await
      // We will use a V4 synchronous message interaction for the test
      .synchronous_message_interaction("calculate rectangle area request", |mut i| async move {
        let proto_file = Path::new("../proto/area_calculator.proto")
          .canonicalize().unwrap().to_string_lossy().to_string();
        // We need to pass all the details for the interaction over to the plugin
        i.contents_from(json!({
          // Configure the proto file, the content type and the service we expect to invoke
          "pact:proto": proto_file,
          "pact:content-type": "application/protobuf",
          "pact:proto-service": "Calculator/calculateOne",

          // Details on the request message (ShapeMessage) we will send
          "request": {
            "rectangle": {
              "length": "matching(number, 3)",
              "width": "matching(number, 4)"
            }
          },

          // Details on the response message we expect to get back (AreaResponse)
          "response": {
            "value": "matching(number, 12)"
          }
        })).await;
        i
      })
      .await
      // Start a mock server using gRPC transport
      .start_mock_server_async(Some("protobuf/transport/grpc"), None)
      .await;

    // Configure the generated calculator client to connect to the gRPC mock server
    let url = mock_server.url();
    let mut client = calculator_client::CalculatorClient::connect(url.to_string()).await.unwrap();

    // Correct request
    let shape_message = ShapeMessage {
      shape: Some(shape_message::Shape::Rectangle(Rectangle {
        length: 4.0,
        width: 4.0
      }))
    };
    let response = client.calculate_one(tonic::Request::new(shape_message)).await;
    let area_message = response.unwrap();
    expect!(area_message.get_ref().value.get(0).unwrap()).to(be_equal_to(&12.0));

    // Incorrect request, missing the length field. Uncommenting this will cause the test to fail.
    // let shape_message = ShapeMessage {
    //   shape: Some(shape_message::Shape::Rectangle(Rectangle {
    //     width: 4.0, .. Rectangle::default()
    //   }))
    // };
    // client.calculate_one(tonic::Request::new(shape_message)).await;
  }
}
