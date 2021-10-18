use core::pin::Pin;
use core::task::{Context, Poll};
use std::collections::HashMap;
use std::io;
use std::io::{Read, Write};
use std::net::SocketAddr;

use anyhow::anyhow;
use csv::{Reader, ReaderBuilder, StringRecord};
use env_logger::Env;
use futures::Stream;
use log::debug;
use maplit::hashmap;
use pact_matching::matchers::Matches;
use pact_models::matchingrules::{MatchingRule, RuleList, RuleLogic};
use pact_models::prelude::ContentType;
use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};
use tonic::{Response, transport::Server};
use uuid::Uuid;

use crate::csv_content::{generate_csv_content, has_headers, setup_csv_contents};
use crate::proto::body::ContentTypeHint;
use crate::proto::catalogue_entry::EntryType;
use crate::proto::pact_plugin_server::{PactPlugin, PactPluginServer};
use crate::proto::to_object;

mod proto;
mod parser;
mod utils;
mod csv_content;

#[derive(Debug, Default)]
pub struct CsvPactPlugin {}

#[tonic::async_trait]
impl PactPlugin for CsvPactPlugin {

  // Returns the catalogue entries for CSV
  async fn init_plugin(
    &self,
    request: tonic::Request<proto::InitPluginRequest>,
  ) -> Result<tonic::Response<proto::InitPluginResponse>, tonic::Status> {
    let message = request.get_ref();
    debug!("Init request from {}/{}", message.implementation, message.version);
    Ok(Response::new(proto::InitPluginResponse {
      catalogue: vec![
        proto::CatalogueEntry {
          r#type: EntryType::ContentMatcher as i32,
          key: "csv".to_string(),
          values: hashmap! {
            "content-types".to_string() => "text/csv;application/csv".to_string()
          }
        },
        proto::CatalogueEntry {
          r#type: EntryType::ContentGenerator as i32,
          key: "csv".to_string(),
          values: hashmap! {
            "content-types".to_string() => "text/csv;application/csv".to_string()
          }
        }
      ]
    }))
  }

  // Not used
  async fn update_catalogue(
    &self,
    _request: tonic::Request<proto::Catalogue>,
  ) -> Result<tonic::Response<()>, tonic::Status> {
    debug!("Update catalogue request, ignoring");
    Ok(Response::new(()))
  }

  // Request to compare the CSV contents
  async fn compare_contents(
    &self,
    request: tonic::Request<proto::CompareContentsRequest>,
  ) -> Result<tonic::Response<proto::CompareContentsResponse>, tonic::Status> {
    let request = request.get_ref();
    debug!("compare_contents request - {:?}", request);

    let has_headers = has_headers(&request.plugin_configuration);

    match (request.expected.as_ref(), request.actual.as_ref()) {
      (Some(expected), Some(actual)) => {
        let expected_csv_data = expected.content.as_ref().unwrap();
        let mut expected_rdr = ReaderBuilder::new().has_headers(has_headers)
          .from_reader(expected_csv_data.as_slice());
        let actual_csv_data = actual.content.as_ref().unwrap();
        let mut actual_rdr = ReaderBuilder::new().has_headers(has_headers)
          .from_reader(actual_csv_data.as_slice());

        let rules = request.rules.iter()
          .map(|(key, rules)| {
            let rules = rules.rule.iter().fold(RuleList::empty(RuleLogic::And), |mut list, rule| {
              match to_object(&rule.values.as_ref().unwrap()) {
                Value::Object(mut map) => {
                  map.insert("match".to_string(), Value::String(rule.r#type.clone()));
                  debug!("Creating matching rule with {:?}", map);
                  list.add_rule(&MatchingRule::from_json(&Value::Object(map)).unwrap());
                }
                _ => {}
              }
              list
            });
            (key.clone(), rules)
          }).collect();
        compare_contents(has_headers, &mut expected_rdr, &mut actual_rdr,
                         request.allow_unexpected_keys, rules)
          .map_err(|err| tonic::Status::aborted(format!("Failed to compare CSV contents: {}", err)))
      }
      (None, Some(actual)) => {
        let contents = actual.content.as_ref().unwrap();
        Ok(Response::new(proto::CompareContentsResponse {
          error: String::default(),
          type_mismatch: None,
          results: hashmap! {
            String::default() => proto::ContentMismatches {
              mismatches: vec![
                proto::ContentMismatch {
                  expected: None,
                  actual: Some(contents.clone()),
                  mismatch: format!("Expected no CSV content, but got {} bytes", contents.len()),
                  path: "".to_string(),
                  diff: "".to_string()
                }
              ]
            }
          }
        }))
      }
      (Some(expected), None) => {
        let contents = expected.content.as_ref().unwrap();
        Ok(Response::new(proto::CompareContentsResponse {
          error: String::default(),
          type_mismatch: None,
          results: hashmap! {
            String::default() => proto::ContentMismatches {
              mismatches: vec![
                proto::ContentMismatch {
                  expected: Some(contents.clone()),
                  actual: None,
                  mismatch: format!("Expected CSV content, but did not get any"),
                  path: "".to_string(),
                  diff: "".to_string()
                }
              ]
            }
          }
        }))
      }
      (None, None) => {
        Ok(Response::new(proto::CompareContentsResponse {
          error: String::default(),
          type_mismatch: None,
          results: hashmap!{}
        }))
      }
    }
  }

  // Request to configure the interaction with CSV contents
  // Example definition we should receive:
  // "column:1", "matching(type,'Name')",
  // "column:2", "matching(number,100)",
  // "column:3", "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
  async fn configure_interaction(
    &self,
    request: tonic::Request<proto::ConfigureInteractionRequest>,
  ) -> Result<tonic::Response<proto::ConfigureInteractionResponse>, tonic::Status> {
    debug!("Received configure_contents request for '{}'", request.get_ref().content_type);
    setup_csv_contents(&request)
      .map_err(|err| tonic::Status::aborted(format!("Invalid column definition: {}", err)))
  }

  // Request to generate CSV contents
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
            content_type_hint: ContentTypeHint::Default as i32
          })
        })
      })
      .map_err(|err| tonic::Status::aborted(format!("Failed to generate CSV contents: {}", err)))
  }
}

fn compare_contents<R: Read>(
  has_headers: bool,
  expected: &mut Reader<R>,
  actual: &mut Reader<R>,
  allow_unexpected_keys: bool,
  rules: HashMap<String, RuleList>
) -> anyhow::Result<tonic::Response<proto::CompareContentsResponse>> {
  debug!("Comparing contents using allow_unexpected_keys ({}) and rules ({:?})", allow_unexpected_keys, rules);

  let mut results = vec![];

  let expected_headers = match expected.headers() {
    Ok(headers) => headers.clone(),
    Err(err) => if has_headers {
      return Err(anyhow!("Failed to read the expected headers: {}", err))
    } else {
      debug!("Failed to read the expected headers: {}", err);
      StringRecord::default()
    }
  };
  let actual_headers = match actual.headers() {
    Ok(headers) => headers.clone(),
    Err(err) => if has_headers {
      return Err(anyhow!("Failed to read the actual headers: {}", err))
    } else {
      debug!("Failed to read the actual headers: {}", err);
      StringRecord::default()
    }
  };
  let actual_headers: HashMap<&str, usize> = actual_headers
    .iter()
    .enumerate()
    .map(|(col, hdr)| (hdr, col))
    .collect();

  if has_headers {
    for header in expected_headers.iter() {
      if !actual_headers.contains_key(header) {
        results.push(proto::ContentMismatch {
          expected: Some(header.as_bytes().to_vec()),
          actual: None,
          mismatch: format!("Expected columns '{}', but was missing", header),
          path: String::default(),
          diff: String::default()
        });
      }
    }
  }

  let mut expected_records = expected.records();
  let mut actual_records = actual.records();

  let expected_row = expected_records.next()
    .ok_or_else(|| anyhow!("Could not read the expected content"))??;
  let actual_row = actual_records.next()
    .ok_or_else(|| anyhow!("Could not read the expected content"))??;

  if !has_headers {
    if actual_row.len() < expected_row.len() {
      results.push(proto::ContentMismatch {
        expected: Some(format!("{} columns", expected_row.len()).as_bytes().to_vec()),
        actual: Some(format!("{} columns", actual_row.len()).as_bytes().to_vec()),
        mismatch: format!("Expected {} columns, but got {}", expected_row.len(), actual_row.len()),
        path: String::default(),
        diff: String::default()
      });
    } else if actual_row.len() > expected_row.len() && !allow_unexpected_keys {
      results.push(proto::ContentMismatch {
        expected: Some(format!("{} columns", expected_row.len()).as_bytes().to_vec()),
        actual: Some(format!("{} columns", actual_row.len()).as_bytes().to_vec()),
        mismatch: format!("Expected at least {} columns, but got {}", expected_row.len(), actual_row.len()),
        path: String::default(),
        diff: String::default()
      });
    }
  }

  compare_row(&expected_row, &actual_row, &rules, has_headers, &expected_headers, &actual_headers, &mut results);
  for row in actual_records {
    compare_row(&expected_row, &row?, &rules, has_headers, &expected_headers, &actual_headers, &mut results);
  }

  Ok(Response::new(proto::CompareContentsResponse {
    error: String::default(),
    type_mismatch: None,
    results: hashmap! {
      String::default() => proto::ContentMismatches {
        mismatches: results
      }
    }
  }))
}

fn compare_row(
  expected_row: &StringRecord,
  actual_row: &StringRecord,
  rules: &HashMap<String, RuleList>,
  has_headers: bool,
  expected_headers: &StringRecord,
  actual_headers: &HashMap<&str, usize>,
  results: &mut Vec<proto::ContentMismatch>) {
  for (index, expected_item) in expected_row.iter().enumerate() {
    let header = expected_headers.get(index).unwrap_or_default();
    let item = if has_headers {
      match actual_headers.get(header) {
        Some(actual_index) => actual_row.get(*actual_index).unwrap_or_default(),
        None => ""
      }
    } else {
      actual_row.get(index).unwrap_or_default()
    };

    let path = format!("column:{}", index + 1);
    let header_path = format!("column:{}", header);

    if let Some(rules) = rules.get(&path).or_else(|| rules.get(header_path.as_str())) {
      for rule in &rules.rules {
        if let Err(err) = expected_item.matches_with(item, rule, false) {
          results.push(proto::ContentMismatch {
            expected: Some(expected_item.as_bytes().to_vec()),
            actual: Some(item.as_bytes().to_vec()),
            mismatch: err.to_string(),
            path: format!("row:{:5}, column:{:2}", actual_row.position().unwrap().line(), index),
            diff: String::default()
          });
        }
      }
    } else if item != expected_item {
      results.push(proto::ContentMismatch {
        expected: Some(expected_item.as_bytes().to_vec()),
        actual: Some(item.as_bytes().to_vec()),
        mismatch: format!("Expected column {} value to equal '{}', but got '{}'", index, expected_item, item),
        path: format!("row:{:5}, column:{:2}", actual_row.position().unwrap().line(), index),
        diff: String::default()
      });
    }
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
  let _ = io::stdout().flush();

  let plugin = CsvPactPlugin::default();
  Server::builder()
    .add_service(PactPluginServer::new(plugin))
    .serve_with_incoming(TcpIncoming { inner: listener }).await?;

  Ok(())
}
