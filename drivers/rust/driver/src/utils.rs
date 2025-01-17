//! Utils for dealing with protobufs structs

use std::collections::{BTreeMap, HashMap};
use anyhow::bail;
use itertools::Itertools;
use os_info::Type;

use prost_types::{ListValue, Struct};
use prost_types::value::Kind;
use semver::{Version, VersionReq};
use serde_json::{json, Value};
use tracing::debug;

/// Converts a map of key -> JSON to a prost Struct
pub fn to_proto_struct(values: &HashMap<String, Value>) -> Struct {
  Struct {
    fields: values.iter().map(|(k, v)| (k.clone(), to_proto_value(v))).collect()
  }
}

/// Converts a JSON value to a prost struct
pub fn to_proto_value(val: &Value) -> prost_types::Value {
  match val {
    Value::Null => prost_types::Value { kind: Some(prost_types::value::Kind::NullValue(0)) },
    Value::Bool(b) => prost_types::Value { kind: Some(prost_types::value::Kind::BoolValue(*b)) },
    Value::Number(n) => if let Some(n) = n.as_u64() {
      prost_types::Value { kind: Some(prost_types::value::Kind::NumberValue(n as f64)) }
    } else if let Some(n) = n.as_i64() {
      prost_types::Value { kind: Some(prost_types::value::Kind::NumberValue(n as f64)) }
    } else {
      prost_types::Value { kind: Some(prost_types::value::Kind::NumberValue(n.as_f64().unwrap_or_default())) }
    }
    Value::String(s) => prost_types::Value { kind: Some(prost_types::value::Kind::StringValue(s.clone())) },
    Value::Array(a) => prost_types::Value { kind: Some(prost_types::value::Kind::ListValue(ListValue {
      values: a.iter().map(|v| to_proto_value(v)).collect()
    }))},
    Value::Object(o) => prost_types::Value { kind: Some(prost_types::value::Kind::StructValue(Struct {
      fields: o.iter().map(|(k, v)| (k.clone(), to_proto_value(v))).collect()
    }))}
  }
}

/// Converts a prost struct to JSON value
pub fn proto_struct_to_json(val: &prost_types::Struct) -> Value {
  Value::Object(val.fields.iter()
    .map(|(k, v)| (k.clone(), proto_value_to_json(v))).collect())
}

/// Converts a prost value to JSON value
pub fn proto_value_to_json(val: &prost_types::Value) -> Value {
  match &val.kind {
    Some(kind) => match kind {
      Kind::NullValue(_) => Value::Null,
      Kind::NumberValue(n) => json!(n),
      Kind::StringValue(s) => Value::String(s.clone()),
      Kind::BoolValue(b) => Value::Bool(*b),
      Kind::StructValue(s) => proto_struct_to_json(s),
      Kind::ListValue(l) => Value::Array(l.values.iter()
        .map(|v| proto_value_to_json(v)).collect())
    }
    None => Value::Null
  }
}

/// Converts a prost struct to a map of key -> JSON
pub fn proto_struct_to_map(val: &prost_types::Struct) -> BTreeMap<String, Value> {
  val.fields.iter()
    .sorted_by(|(k1, _), (k2, _)| Ord::cmp(k1, k2))
    .map(|(k, v)| (k.clone(), proto_value_to_json(v)))
    .collect()
}

/// Converts a prost struct to a hash map of key -> JSON
pub fn proto_struct_to_hashmap(val: &prost_types::Struct) -> HashMap<String, Value> {
  val.fields.iter()
    .map(|(k, v)| (k.clone(), proto_value_to_json(v)))
    .collect()
}

/// Convert a proto struct into a String
pub fn proto_value_to_string(val: &prost_types::Value) -> Option<String> {
  match &val.kind {
    Some(kind) => match kind {
      Kind::NullValue(_) => None,
      Kind::NumberValue(n) => Some(n.to_string()),
      Kind::StringValue(s) => Some(s.clone()),
      Kind::BoolValue(b) => Some(b.to_string()),
      Kind::StructValue(s) => Some(proto_struct_to_json(s).to_string()),
      Kind::ListValue(l) => Some(Value::Array(l.values.iter()
        .map(|v| proto_value_to_json(v)).collect()).to_string())
    }
    None => None
  }
}

/// Check if the versions are compatible (differ in patch version only)
pub fn versions_compatible(version: &str, required: &Option<String>) -> bool {
  match required {
    None => true,
    Some(required) => {
      if required == version {
        true
      } else if let Ok(version) = Version::parse(version) {
        if let Ok(req) = VersionReq::parse(format!(">={}", required).as_str()) {
          req.matches(&version)
        } else {
          false
        }
      } else {
        false
      }
    }
  }
}

pub fn optional_string<S: Into<String>>(string: S) -> Option<String> {
  let string = string.into();
  if string.is_empty() {
    None
  } else {
    Some(string)
  }
}

/// Returns the current running OS and architecture
pub fn os_and_arch() -> anyhow::Result<(&'static str, &'static str)> {
  let os_info = os_info::get();
  debug!("Detected OS: {}", os_info);

  let os = match os_info.os_type() {
    Type::Alpine | Type::Amazon| Type::Android| Type::Arch| Type::CentOS| Type::Debian |
    Type::EndeavourOS | Type::Fedora | Type::Gentoo | Type::Linux | Type::Manjaro | Type::Mariner |
    Type::Mint | Type::NixOS | Type::openSUSE | Type::OracleLinux | Type::Redhat |
    Type::RedHatEnterprise | Type::Pop | Type::Raspbian | Type::Solus | Type::SUSE |
    Type::Ubuntu => "linux",
    Type::Macos => "osx",
    Type::Windows => "windows",
    _ => bail!("{} is not a supported operating system", os_info)
  };

  Ok((os, std::env::consts::ARCH))
}

#[cfg(test)]
mod tests {
  use expectest::prelude::*;

  use super::versions_compatible;

  #[test]
  fn versions_compatible_test() {
    expect!(versions_compatible("1.0.0", &None)).to(be_true());
    expect!(versions_compatible("1.0.0", &Some("1.0.0".to_string()))).to(be_true());
    expect!(versions_compatible("1.0.0", &Some("1.0.1".to_string()))).to(be_false());
    expect!(versions_compatible("1.0.4", &Some("1.0.3".to_string()))).to(be_true());
    expect!(versions_compatible("1.1.0", &Some("1.0.3".to_string()))).to(be_true());
    expect!(versions_compatible("2.0.1", &Some("1.1.0".to_string()))).to(be_true());
    expect!(versions_compatible("1.0.1", &Some("2.1.0".to_string()))).to(be_false());
    expect!(versions_compatible("0.1.0", &Some("0.0.3".to_string()))).to(be_true());
  }
}
