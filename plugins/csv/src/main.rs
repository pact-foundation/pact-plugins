use core::pin::Pin;
use core::task::{Context, Poll};
use std::net::SocketAddr;

use anyhow::anyhow;
use csv::{Writer, ReaderBuilder};
use env_logger::Env;
use futures::Stream;
use log::debug;
use maplit::hashmap;
use pact_models::generators::{NoopVariantMatcher, VariantMatcher, GenerateValue};
use pact_models::prelude::{Generator, ContentType};
use prost_types::value::Kind;
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tonic::{Request, Response, transport::Server};
use uuid::Uuid;
use bytes::Bytes;

use proto::pact_plugin_server::{PactPlugin, PactPluginServer};

use crate::parser::{parse_field, parse_value};
use pact_models::bodies::OptionalBody;

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

fn generate_csv_content(request: &Request<proto::GenerateContentRequest>) -> anyhow::Result<OptionalBody> {
  let mut generators = hashmap! {};
  for (key, gen) in &request.get_ref().generators {
    let column = parse_field(&key)?;
    let values = gen.values.as_ref().ok_or(anyhow!("Generator values were expected"))?.fields.iter().map(|(k, v)| {
      (k.clone(), from_value(v))
    }).collect();
    let generator = Generator::from_map(&gen.r#type, &values)
      .ok_or(anyhow!("Failed to build generator of type {}", gen.r#type))?;
    generators.insert(column, generator);
  };

  let context = hashmap! {};
  let variant_matcher = NoopVariantMatcher.boxed();
  let mut wtr = Writer::from_writer(vec![]);
  let csv_data = request.get_ref().contents.as_ref().unwrap().content.as_ref().unwrap();
  let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(csv_data.as_slice());
  for result in rdr.records() {
    let record = result?;
    for (col, field) in record.iter().enumerate() {
      debug!("got column:{} = '{}'", col, field);
      if let Some(generator) = generators.get(&col) {
        let value = generator.generate_value(&field.to_string(), &context, &variant_matcher)?;
        wtr.write_field(value)?;
      } else {
        wtr.write_field(field)?;
      }
    }
    wtr.write_record(None::<&[u8]>)?;
  }
  let generated = wtr.into_inner()?;
  debug!("Generated contents has {} bytes", generated.len());
  let bytes = Bytes::from(generated);
  Ok(OptionalBody::Present(bytes, Some(ContentType::from("text/csv;charset=UTF-8"))))
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

fn from_value(value: &prost_types::Value) -> Value {
  match value.kind.as_ref().unwrap() {
    Kind::NullValue(_) => Value::Null,
    Kind::NumberValue(n) => json!(*n),
    Kind::StringValue(s) => Value::String(s.clone()),
    Kind::BoolValue(b) => Value::Bool(*b),
    Kind::StructValue(s) => Value::Object(s.fields.iter()
      .map(|(k, v)| (k.clone(), from_value(v))).collect()),
    Kind::ListValue(l) => Value::Array(l.values.iter()
      .map(|v| from_value(v)).collect())
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
        },
        proto::CatalogueEntry {
          r#type: "content-generator".to_string(),
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

  async fn generate_content(
    &self,
    request: tonic::Request<proto::GenerateContentRequest>,
  ) -> Result<tonic::Response<proto::GenerateContentResponse>, tonic::Status> {
    debug!("Received generate_content request");

    generate_csv_content(&request)
      .map(|contents| {
        debug!("Generated contents: {}", contents);
        Response::new(proto::GenerateContentResponse {
          contents: Some(proto::Body {
            content_type: contents.content_type().unwrap_or(ContentType::from("text/csv")).to_string(),
            content: Some(contents.value().unwrap().to_vec()),
          })
        })
      })
      .map_err(|err| tonic::Status::aborted(format!("Failed to generate CSV contents: {}", err)))
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
