use core::pin::Pin;
use core::task::{Context, Poll};
use std::convert::TryInto;
use std::net::SocketAddr;

use futures::Stream;
use futures::stream::StreamExt;
use tokio::net::{TcpListener, TcpStream};
use tonic::{Request, Response, Status, transport::Server};
use uuid::Uuid;

use proto::pact_plugin_server::{PactPlugin, PactPluginServer};

mod proto;

#[derive(Debug, Default)]
pub struct CsvPactPlugin {}

#[tonic::async_trait]
impl PactPlugin for CsvPactPlugin {
  async fn init_plugin(
    &self,
    request: tonic::Request<proto::InitPluginRequest>,
  ) -> Result<tonic::Response<proto::InitPluginResponse>, tonic::Status> {
    Err(tonic::Status::unimplemented("unimplemented"))
  }

  async fn update_catalogue(
    &self,
    request: tonic::Request<proto::Catalogue>,
  ) -> Result<tonic::Response<proto::Void>, tonic::Status> {
    Err(tonic::Status::unimplemented("unimplemented"))
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
  let addr: SocketAddr = "0.0.0.0:0".parse()?;
  let listener = TcpListener::bind(addr).await?;
  let address = listener.local_addr()?;

  println!("{{\"port\":{}, \"serverKey\":\"{}\"}}", address.port(), Uuid::new_v4().to_string());

  let plugin = CsvPactPlugin::default();
  Server::builder()
    .add_service(PactPluginServer::new(plugin))
    .serve_with_incoming(TcpIncoming { inner: listener }).await;

  Ok(())
}
