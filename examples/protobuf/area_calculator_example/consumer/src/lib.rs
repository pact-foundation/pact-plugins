use std::f32::consts::PI;

tonic::include_proto!("area_calculator");

struct CalculatorImplementation {}

#[tonic::async_trait]
impl calculator_server::Calculator for CalculatorImplementation {
  async fn calculate(&self, request: tonic::Request<ShapeMessage>) -> Result<tonic::Response<AreaResponse>, tonic::Status> {
    let shape_message = request.into_inner();

    // Make sure the generators are working
    if shape_message.created == "2000-01-01" {
      return Err(tonic::Status::failed_precondition(format!("Invalid created date '{}'", shape_message.created)));
    }

    let area = if let Some(shape) = shape_message.shape {
      match shape {
        shape_message::Shape::Square(s) => {
          s.edge_length * s.edge_length
        }
        shape_message::Shape::Rectangle(r) => {
          r.length * r.width
        }
        shape_message::Shape::Circle(c) => {
          PI * c.radius * c.radius
        }
        shape_message::Shape::Triangle(t) => {
          let p = (t.edge_a + t.edge_b + t.edge_c) / 2.0;
          f32::sqrt(p * (p - t.edge_a) * (p - t.edge_b) * (p - t.edge_c))
        }
        shape_message::Shape::Parallelogram(p) => {
          p.base_length * p.height
        }
      }
    } else {
      0.0
    };

    Ok(tonic::Response::new(AreaResponse { value: area }))
  }
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use bytes::BytesMut;
  use expectest::prelude::*;
  use maplit::hashmap;
  use pact_consumer::prelude::*;
  use pact_models::generators::GeneratorTestMode;
  use pact_matching::generators::apply_generators_to_sync_message;
  use prost::Message;
  use reqwest::Client;
  use serde_json::json;

  use crate::calculator_server::Calculator;

  use super::*;

  #[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 1))]
  async fn test_area_calculator_client_with_message() {
    let mut pact_builder = &mut PactBuilderAsync::new_v4("area_calculator-consumer", "area_calculator-provider");
    pact_builder = pact_builder.using_plugin("protobuf", None).await;
    let proto_service = pact_builder
      .synchronous_message_interaction("request for calculate shape area", |mut i| async move {
        let project_dir = Path::new(option_env!("CARGO_MANIFEST_DIR").unwrap());
        let proto_file = project_dir.join("..").join("proto").join("area_calculator.proto");

        i.contents_from(json!({
          "pact:proto": proto_file.to_str().unwrap(),
          "pact:content-type": "application/protobuf",
          "pact:proto-service": "Calculator/calculate",
          "request": {
            "rectangle": {
              "length": "matching(number, 3)",
              "width": "matching(number, 4)"
            },
            "created": "matching(date, 'yyyy-MM-dd', '2000-01-01')"
          },
          "response": {
            "value" : "matching(number, 12)"
          }
        })).await;
        i.test_name("area_calculator::test::test_area_calculator_client");
        i
      })
      .await;

    let pact = proto_service.build();
    for message in proto_service.synchronous_messages() {
      let (request_contents, response_contents) = apply_generators_to_sync_message(
        &message,
        &GeneratorTestMode::Consumer,
        &hashmap!{},
        &pact.plugin_data(),
        &message.plugin_config
      ).await;
      let bytes = request_contents.contents.value().unwrap();
      let request = ShapeMessage::decode(bytes).unwrap();
      let bytes = response_contents.first().unwrap().contents.value().unwrap();
      let response = AreaResponse::decode(bytes).unwrap();
      let calculator = CalculatorImplementation { };

      let result = calculator.calculate(tonic::Request::new(request)).await;

      expect!(result.as_ref()).to(be_ok());
      expect!(result.unwrap().into_inner().value).to(be_equal_to(response.value));
    }
  }

  #[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 1))]
  async fn test_area_calculator_client_via_http() {
    let project_dir = Path::new(option_env!("CARGO_MANIFEST_DIR").unwrap());
    let proto_file = project_dir.join("..").join("proto").join("area_calculator.proto");

    let mock_service = PactBuilder::new_v4("area_calculator-consumer", "area_calculator-provider")
      .using_plugin("protobuf", None).await
      .interaction("request for calculate shape area via http", "", |mut interaction| async {
        interaction
          .request
          .post()
          .path("/calculate")
          .contents("application/protobuf".into(), json!({
            "pact:proto": proto_file.to_str().unwrap(),
            "pact:proto-service": "Calculator/calculate:request",
            "rectangle": {
              "length": "matching(number, 3)",
              "width": "matching(number, 4)"
            }
          }))
          .await;
        interaction.response.contents("application/protobuf".into(), json!({
            "pact:proto": proto_file.to_str().unwrap(),
            "pact:proto-service": "Calculator/calculate:response",
            "value": "matching(number, 12)"
          })).await;

        interaction
      })
      .await
      .start_mock_server(None);

    let mock_url = mock_service.url();

    let shape = ShapeMessage {
      shape: Some(shape_message::Shape::Rectangle(Rectangle { length: 3.0, width: 4.0 })),
      .. ShapeMessage::default()
    };
    let mut buffer = BytesMut::new();
    shape.encode(&mut buffer).unwrap();
    let response = Client::new()
      .post(format!("{}calculate", mock_url))
      .header("content-type", "application/protobuf;message=.area_calculator.ShapeMessage")
      .body(buffer.freeze())
      .send()
      .await
      .unwrap()
      .bytes()
      .await
      .unwrap();
    let area = AreaResponse::decode(response).unwrap();
    expect!(area.value).to(be_equal_to(12.0));
  }
}
