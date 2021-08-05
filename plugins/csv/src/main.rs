use core::pin::Pin;
use core::task::{Context, Poll};
use std::net::SocketAddr;

use anyhow::anyhow;
use csv::Writer;
use env_logger::Env;
use futures::Stream;
use log::debug;
use maplit::hashmap;
use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};
use tonic::{Request, Response, Status, transport::Server};
use uuid::Uuid;

use proto::pact_plugin_server::{PactPlugin, PactPluginServer};

use crate::parser::{parse_field, parse_value};

mod proto;
mod parser;

#[derive(Debug, Default)]
pub struct CsvPactPlugin {}

fn setup_csv_contents(request: &Request<proto::ConfigureContentsRequest>) -> anyhow::Result<Response<proto::ConfigureContentsResponse>> {
  match &request.get_ref().contents_config {
    Some(config) => {
      let mut columns = vec![];
      for (key, value) in &config.fields {
        let column = parse_field(&key)?;
        let result = parse_value(&value)?;
        debug!("Parsed column definition: {}, {:?}", column, result);
        if column > columns.len() {
          columns.resize(column, None)
        }
        columns[column - 1] = Some(result);
      }
      let mut wtr = Writer::from_writer(vec![]);
      let column_values = columns.iter().map(|v| {
        if let Some(v) = v {
          &v.0
        } else {
          ""
        }
      }).collect::<Vec<&str>>();
      wtr.write_record(column_values)?;
      let mut rules = hashmap!{};
      let mut generators = hashmap!{};
      for (col, vals) in columns.iter().enumerate() {
        if let Some((_, rule, gen)) = vals {
          if let Some(rule) = rule {
            debug!("rule.values()={:?}", rule.values());
            rules.insert(format!("column:{}", col), proto::MatchingRules {
              rule: vec![
                proto::MatchingRule {
                  r#type: rule.name(),
                  values: Some(prost_types::Struct {
                    fields: rule.values().iter().map(|(key, val)| (key.to_string(), to_value(val))).collect()
                  })
                }
              ]
            });
          }
          if let Some(gen) = gen {
            generators.insert(format!("column:{}", col), proto::Generator {
              r#type: gen.name(),
              values: Some(prost_types::Struct {
                fields: gen.values().iter().map(|(key, val)| (key.to_string(), to_value(val))).collect()
              })
            });
          }
        }
      }
      debug!("matching rules = {:?}", rules);
      debug!("generators = {:?}", generators);
      Ok(Response::new(proto::ConfigureContentsResponse {
        contents: Some(proto::Body {
          content_type: "text/csv;charset=UTF-8".to_string(),
          content: Some(wtr.into_inner()?),
        }),
        rules,
        generators
      }))
    }
    None => Err(anyhow!("No config provided to match/generate CSV content"))
  }
}

fn to_value(value: &Value) -> prost_types::Value {
  match value {
    Value::Null => prost_types::Value { kind: Some(prost_types::value::Kind::NullValue(0)) },
    Value::Bool(b) => prost_types::Value { kind: Some(prost_types::value::Kind::BoolValue(*b)) },
    Value::Number(n) => if n.is_u64() {
      prost_types::Value { kind: Some(prost_types::value::Kind::NumberValue(n.as_u64().unwrap_or_default() as f64)) }
    } else if n.is_i64() {
      prost_types::Value { kind: Some(prost_types::value::Kind::NumberValue(n.as_i64().unwrap_or_default() as f64)) }
    } else {
      prost_types::Value { kind: Some(prost_types::value::Kind::NumberValue(n.as_f64().unwrap_or_default())) }
    }
    Value::String(s) => prost_types::Value { kind: Some(prost_types::value::Kind::StringValue(s.clone())) },
    Value::Array(v) => prost_types::Value { kind: Some(prost_types::value::Kind::ListValue(prost_types::ListValue {
      values: v.iter().map(|val| to_value(val)).collect()
    })) },
    Value::Object(m) => prost_types::Value { kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
      fields: m.iter().map(|(key, val)| (key.clone(), to_value(val))).collect()
    })) }
  }
}

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
            "content-types".to_string() => "text/csv;application/csv".to_string()
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

  async fn configure_contents(
    &self,
    request: tonic::Request<proto::ConfigureContentsRequest>,
  ) -> Result<tonic::Response<proto::ConfigureContentsResponse>, tonic::Status> {
    debug!("Received configure_contents request for '{}'", request.get_ref().content_type);

    // "column:1", "matching(type,'Name')",
    // "column:2", "matching(number,100)",
    // "column:3", "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
    setup_csv_contents(&request)
      .map_err(|err| tonic::Status::aborted(format!("Invalid column definition: {}", err)))
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
