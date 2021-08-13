use prost_types::value::Kind;
use serde_json::{json, Value};

tonic::include_proto!("io.pact.plugin");

pub fn to_object(s: &prost_types::Struct) -> Value {
  Value::Object(s.fields.iter()
    .map(|(k, v)| (k.clone(), to_value(v)))
    .collect())
}

pub fn to_value(v: &prost_types::Value) -> Value {
  match &v.kind {
    Some(kind) => match kind {
      Kind::NullValue(_) => Value::Null,
      Kind::NumberValue(n) => json!(n),
      Kind::StringValue(s) => Value::String(s.clone()),
      Kind::BoolValue(b) => Value::Bool(*b),
      Kind::StructValue(s) => to_object(s),
      Kind::ListValue(l) => Value::Array(l.values.iter().map(|v| to_value(v)).collect())
    }
    None => Value::Null
  }
}
