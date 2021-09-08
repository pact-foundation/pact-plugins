use prost_types::value::Kind;
use serde_json::{json, Value};

pub fn to_value(value: &Value) -> prost_types::Value {
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

pub fn from_value(value: &prost_types::Value) -> Value {
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

pub fn to_boolean(value: &prost_types::Value) -> bool {
  match value.kind.as_ref().unwrap() {
    Kind::NullValue(_) => false,
    Kind::NumberValue(n) => *n == 0.0,
    Kind::StringValue(s) => !s.is_empty(),
    Kind::BoolValue(b) => *b,
    Kind::StructValue(s) => !s.fields.is_empty(),
    Kind::ListValue(l) => !l.values.is_empty()
  }
}
