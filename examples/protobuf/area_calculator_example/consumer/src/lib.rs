use std::f32::consts::PI;

tonic::include_proto!("area_calculator");

struct CalculatorImplementation {}

#[tonic::async_trait]
impl calculator_server::Calculator for CalculatorImplementation {
  async fn calculate(&self, request: tonic::Request<ShapeMessage>) -> Result<tonic::Response<AreaResponse>, tonic::Status> {
    let shape_message = request.into_inner();
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

  use expectest::prelude::*;
  use pact_consumer::prelude::*;
  use prost::Message;
  use serde_json::json;

  use crate::calculator_server::Calculator;

  use super::*;

  #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
  async fn test_area_calculator_client() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut pact_builder = PactBuilder::new_v4("area_calculator-consumer", "area_calculator-provider");
    let proto_service = pact_builder
      .using_plugin("protobuf", None).await
      .synchronous_message_interaction("request for calculate shape area", "core/interaction/synchronous-message", |mut i| async move {
        let project_dir = Path::new(option_env!("CARGO_MANIFEST_DIR").unwrap());
        println!("project_dir = {:?}", project_dir);
        let proto_file = project_dir.join("..").join("area_calculator.proto");
        println!("proto_file = {:?}", proto_file);

        i.contents_from(json!({
          "pact:proto": proto_file.to_str().unwrap(),
          "pact:content-type": "application/protobuf",
          "pact:proto-service": "Calculator/calculate",
          "request": {
            "rectangle": {
              "length": "matching(number, 3)",
              "width": "matching(number, 4)"
            }
          },
          "response": {
            "value" : "matching(number, 12)"
          }
        })).await;
        i.test_name("area_calculator::test::test_area_calculator_client");
        i
      })
      .await;

    for message in proto_service.synchronous_messages() {
      let bytes = message.request.contents.value().unwrap();
      let request = ShapeMessage::decode(bytes).unwrap();
      let bytes = message.response.first().unwrap().contents.value().unwrap();
      let response = AreaResponse::decode(bytes).unwrap();
      let calculator = CalculatorImplementation { };

      let result = calculator.calculate(tonic::Request::new(request)).await;

      expect!(result.as_ref()).to(be_ok());
      expect!(result.unwrap().into_inner().value).to(be_equal_to(response.value));
    }
  }
}
