use core::pin::Pin;
use core::task::{Context, Poll};
use std::net::SocketAddr;

use futures::Stream;
use log::debug;
use tokio::net::{TcpListener, TcpStream};
use tonic::{Response, transport::Server};
use uuid::Uuid;
use maplit::hashmap;
use proto::pact_plugin_server::{PactPlugin, PactPluginServer};
use env_logger::Env;

mod proto;

#[derive(Debug, Default)]
pub struct CsvPactPlugin {}

#[tonic::async_trait]
impl PactPlugin for CsvPactPlugin {
  async fn init_plugin(
    &self,
    request: tonic::Request<proto::InitPluginRequest>,
  ) -> Result<tonic::Response<proto::InitPluginResponse>, tonic::Status> {
    let message = request.get_ref();
    debug!("Init request from {}/{}", message.implementation, message.version);
    Ok(Response::new(proto::InitPluginResponse {
      catalogue: vec![
        proto::CatalogueEntry {
          r#type: "content-matcher".to_string(),
          key: "csv".to_string(),
          values: hashmap! {
            "content-types".to_string() => "text/csv".to_string()
          }
        }
      ]
    }))
  }

  async fn update_catalogue(
    &self,
    request: tonic::Request<proto::Catalogue>,
  ) -> Result<tonic::Response<proto::Void>, tonic::Status> {
    debug!("Update catalogue request, ignoring");
    Ok(Response::new(proto::Void {}))
  }

  async fn compare_contents(
    &self,
    request: tonic::Request<proto::CompareContentsRequest>,
  ) -> Result<tonic::Response<proto::CompareContentsResponse>, tonic::Status> {
    Err(tonic::Status::unimplemented("unimplemented"))
  }
}

struct TcpIncoming {
  inner: TcpListener
}

impl Stream for TcpIncoming {
  type Item = Result<TcpStream, std::io::Error>;

  fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    Pin::new(&mut self.inner).poll_accept(cx)
      .map_ok(|(stream, _)| stream).map(|v| Some(v))
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let env = Env::new().filter("LOG_LEVEL");
  env_logger::init_from_env(env);

  let addr: SocketAddr = "0.0.0.0:0".parse()?;
  let listener = TcpListener::bind(addr).await?;
  let address = listener.local_addr()?;

  let server_key = Uuid::new_v4().to_string();
  println!("{{\"port\":{}, \"serverKey\":\"{}\"}}", address.port(), server_key);

  let plugin = CsvPactPlugin::default();
  Server::builder()
    .add_service(PactPluginServer::new(plugin))
    .serve_with_incoming(TcpIncoming { inner: listener }).await?;

  Ok(())
}
