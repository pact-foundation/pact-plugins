//! Utils for dealing with protobufs

use std::collections::HashMap;

use prost_types::{ListValue, Struct};
use prost_types::value::Kind;
use serde_json::{json, Value};

pub(crate) fn to_proto_struct(values: HashMap<String, Value>) -> Struct {
  Struct {
    fields: values.iter().map(|(k, v)| (k.clone(), to_proto_value(v))).collect()
  }
}

fn to_proto_value(val: &Value) -> prost_types::Value {
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

pub(crate) fn proto_struct_to_json(val: &prost_types::Struct) -> Value {
  Value::Object(val.fields.iter()
    .map(|(k, v)| (k.clone(), proto_value_to_json(v))).collect())
}

pub(crate) fn proto_value_to_json(val: &prost_types::Value) -> Value {
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
