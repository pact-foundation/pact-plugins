use std::f32::consts::PI;

use tonic::{Request, Response, Status};
use tonic::transport::Server;
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use crate::pb::{AreaResponse, ShapeMessage};
use crate::pb::calculator_server::{Calculator, CalculatorServer};
use crate::pb::shape_message::Shape;

pub mod pb {
    tonic::include_proto!("area_calculator");
}

#[derive(Default)]
pub struct AreaCalculator {}

#[tonic::async_trait]
impl Calculator for AreaCalculator {
    async fn calculate(
        &self,
        request: Request<ShapeMessage>
    ) -> Result<Response<AreaResponse>, Status> {
        let shape_message = request.into_inner();
        match shape_message.shape.unwrap() {
            Shape::Square(sq) => {
                Ok(Response::new(AreaResponse { value: vec![ sq.edge_length * sq.edge_length ] }))
            }
            Shape::Rectangle(rect) => {
                Ok(Response::new(AreaResponse { value: vec![ rect.width * rect.length ] }))
            }
            Shape::Circle(c) => {
                Ok(Response::new(AreaResponse { value: vec![ PI * c.radius * c.radius ] }))
            }
            Shape::Triangle(tri) => {
                let p = (tri.edge_a + tri.edge_b + tri.edge_c) / 2.0;
                Ok(Response::new(AreaResponse { value: vec![
                    (p * (p - tri.edge_a) * (p - tri.edge_b) * (p - tri.edge_c)).sqrt()
                ] }))
            }
            Shape::Parallelogram(_) => {
                Err(Status::unimplemented("we do not currently support parallelograms"))
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .pretty()
      .finish();
    if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("WARN: Failed to initialise global tracing subscriber - {err}");
    };

    let addr = "[::1]:23212".parse().unwrap();
    let service = AreaCalculator::default();

    info!("AreaCalculator listening on {}", addr);

    Server::builder()
      .add_service(CalculatorServer::new(service))
      .serve(addr)
      .await?;

    Ok(())
}
