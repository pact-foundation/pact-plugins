use anyhow::{anyhow, bail, Context, Result};
use log::trace;
use serde::{Deserialize, Serialize};
use serde_json::{json, Number, Value};

use crate::proto::{self, ContentMismatch, MetadataValue};

fn default_jsonrpc_version() -> String {
  "2.0".to_string()
}

fn default_path() -> String {
  "/rpc".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequestSpec {
  #[serde(default = "default_jsonrpc_version")]
  pub jsonrpc: String,
  pub method: String,
  #[serde(default)]
  pub params: Value,
  #[serde(default)]
  pub id: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponseSpec {
  #[serde(default = "default_jsonrpc_version")]
  pub jsonrpc: String,
  #[serde(default)]
  pub result: Option<Value>,
  #[serde(default)]
  pub error: Option<Value>,
  #[serde(default)]
  pub id: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcInteractionConfig {
  #[serde(default = "default_path")]
  pub path: String,
  pub request: JsonRpcRequestSpec,
  pub response: JsonRpcResponseSpec,
}

impl JsonRpcInteractionConfig {
  pub fn from_contents_config(value: Value) -> Result<Self> {
    trace!("Configuring interaction config from {:?}", &value);
    let config: Self = serde_json::from_value(value)
      .context("failed to parse JSON-RPC interaction configuration")?;
    config.validate()?;
    Ok(config)
  }

  pub fn validate(&self) -> Result<()> {
    if self.path.is_empty() || !self.path.starts_with('/') {
      bail!("JSON-RPC interactions require a path starting with '/'");
    }

    if self.request.method.trim().is_empty() {
      bail!("JSON-RPC interactions require a non-empty request.method");
    }

    if self.request.jsonrpc != "2.0" {
      bail!("Only JSON-RPC 2.0 is supported");
    }

    if self.response.jsonrpc != "2.0" {
      bail!("Only JSON-RPC 2.0 responses are supported");
    }

    if self.response.result.is_some() == self.response.error.is_some() {
      bail!("JSON-RPC responses must define exactly one of response.result or response.error");
    }

    Ok(())
  }

  pub fn request_json(&self) -> Value {
    normalise_numbers(json!({
      "jsonrpc": self.request.jsonrpc,
      "method": self.request.method,
      "params": self.request.params,
      "id": self.request.id
    }))
  }

  pub fn response_json(&self) -> Value {
    let mut response = json!({
      "jsonrpc": self.response.jsonrpc,
      "id": self.response.id.clone().unwrap_or_else(|| self.request.id.clone())
    });

    if let Some(value) = &self.response.result {
      response["result"] = value.clone();
    }

    if let Some(value) = &self.response.error {
      response["error"] = value.clone();
    }

    normalise_numbers(response)
  }

  pub fn request_body(&self) -> Result<Vec<u8>> {
    serde_json::to_vec_pretty(&self.request_json())
      .context("failed to serialise JSON-RPC request body")
  }

  pub fn response_body(&self) -> Result<Vec<u8>> {
    serde_json::to_vec_pretty(&self.response_json())
      .context("failed to serialise JSON-RPC response body")
  }

  pub fn interaction_metadata(&self) -> Vec<(String, MetadataValue)> {
    vec![
      (
        "request.path".to_string(),
        proto::string_metadata(self.path.clone()),
      ),
      ("request.method".to_string(), proto::string_metadata("POST")),
      (
        "contentType".to_string(),
        proto::string_metadata("application/json"),
      ),
    ]
  }

  pub fn request_mismatches(&self, actual_path: &str, actual: &Value) -> Vec<ContentMismatch> {
    let mut mismatches = vec![];

    if self.path != actual_path {
      mismatches.push(mismatch(
        "$.path",
        &Value::String(self.path.clone()),
        &Value::String(actual_path.to_string()),
        format!(
          "Expected request path '{}' but received '{}'",
          self.path, actual_path
        ),
      ));
    }

    if self.request_json() != *actual {
      mismatches.push(mismatch(
        "$.request",
        &self.request_json(),
        actual,
        "Expected JSON-RPC request to match the configured interaction".to_string(),
      ));
    }

    mismatches
  }

  pub fn response_mismatches(&self, actual: &Value) -> Vec<ContentMismatch> {
    if self.response_json() == *actual {
      vec![]
    } else {
      vec![mismatch(
        "$.response",
        &self.response_json(),
        actual,
        "Expected JSON-RPC response to match the configured interaction".to_string(),
      )]
    }
  }

  pub fn provider_url(
    &self,
    scheme: &str,
    host: &str,
    port: u32,
    override_path: Option<&str>,
  ) -> String {
    format!(
      "{scheme}://{host}:{port}{}",
      override_path.unwrap_or(&self.path)
    )
  }
}

pub fn parse_json_body(body: &[u8], label: &str) -> Result<Value> {
  serde_json::from_slice(body).with_context(|| format!("failed to parse {label} as JSON"))
}

pub fn config_from_struct(
  value: Option<&prost_types::Struct>,
) -> Result<Option<JsonRpcInteractionConfig>> {
  match value {
    Some(value) => Ok(Some(JsonRpcInteractionConfig::from_contents_config(
      crate::proto::proto_struct_to_json(value),
    )?)),
    None => Ok(None),
  }
}

fn mismatch(path: &str, expected: &Value, actual: &Value, message: String) -> ContentMismatch {
  ContentMismatch {
    expected: Some(serde_json::to_vec(expected).unwrap_or_default()),
    actual: Some(serde_json::to_vec(actual).unwrap_or_default()),
    mismatch: message,
    path: path.to_string(),
    diff: String::new(),
    mismatch_type: "body".to_string(),
  }
}

pub fn override_string(
  metadata: &std::collections::HashMap<String, MetadataValue>,
  key: &str,
) -> Result<Option<String>> {
  let Some(value) = metadata.get(key) else {
    return Ok(None);
  };

  match value.value.as_ref() {
    Some(crate::proto::metadata_value::Value::NonBinaryValue(value)) => match value.kind.as_ref() {
      Some(prost_types::value::Kind::StringValue(value)) => Ok(Some(value.clone())),
      _ => Err(anyhow!("metadata '{key}' must be a string")),
    },
    Some(crate::proto::metadata_value::Value::BinaryValue(_)) => {
      Err(anyhow!("metadata '{key}' must not be binary"))
    }
    None => Ok(None),
  }
}

fn normalise_numbers(value: Value) -> Value {
  match value {
    Value::Array(values) => Value::Array(values.into_iter().map(normalise_numbers).collect()),
    Value::Object(values) => Value::Object(
      values
        .into_iter()
        .map(|(key, value)| (key, normalise_numbers(value)))
        .collect(),
    ),
    Value::Number(value) => {
      if let Some(float) = value.as_f64() {
        if float.is_finite()
          && float.fract() == 0.0
          && float >= i64::MIN as f64
          && float <= i64::MAX as f64
        {
          Value::Number(Number::from(float as i64))
        } else {
          Value::Number(value)
        }
      } else {
        Value::Number(value)
      }
    }
    other => other,
  }
}
