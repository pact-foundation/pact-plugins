tonic::include_proto!("area_calculator");

#[cfg(test)]
mod tests {
  use std::path::Path;

  use expectest::prelude::*;
  use pact_consumer::prelude::*;
  use pact_consumer::mock_server::StartMockServerAsync;
  use serde_json::json;
  use tonic::Code;

  use super::*;

  // This example simulates the Parallelogram shape not being implemented, and an UNIMPLEMENTED status is returned
  #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
  async fn test_proto_client() {
    let mut pact_builder = PactBuilderAsync::new_v4("grpc-consumer-rust", "grpc-provider");
    let mock_server = pact_builder
      .using_plugin("protobuf", None).await
      .synchronous_message_interaction("invalid request", |mut i| async move {
        let proto_file = Path::new("../proto/grpc_status.proto")
          .canonicalize().unwrap().to_string_lossy().to_string();
        i.contents_from(json!({
          "pact:proto": proto_file,
          "pact:content-type": "application/protobuf",
          "pact:proto-service": "Calculator/calculate",

          "request": {
            "parallelogram": {
              "base_length": "matching(number, 3)",
              "height": "matching(number, 4)"
            }
          },

          "responseMetadata": {
            "grpc-status": "UNIMPLEMENTED",
            "grpc-message": "we do not currently support parallelograms"
          }
        })).await;
        i
      })
      .await
      .start_mock_server_async(Some("protobuf/transport/grpc"), None)
      .await;

    let url = mock_server.url();
    let mut client = calculator_client::CalculatorClient::connect(url.to_string()).await.unwrap();

    // Correct request
    let shape_message = ShapeMessage {
      shape: Some(shape_message::Shape::Parallelogram(Parallelogram {
        base_length: 3.0,
        height: 4.0,
      }))
    };
    let response = client.calculate(tonic::Request::new(shape_message)).await;
    expect!(response.as_ref()).to(be_err());
    let status = response.unwrap_err();
    expect!(status.code()).to(be_equal_to(Code::Unimplemented));
    expect!(status.message()).to(be_equal_to("we do not currently support parallelograms"));
  }
}
