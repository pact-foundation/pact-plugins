//! Utils for dealing with protobufs

use std::collections::HashMap;

use prost_types::{Struct, ListValue};
use serde_json::Value;

pub(crate) fn to_proto_struct(values: HashMap<&str, Value>) -> Struct {
  Struct {
    fields: values.iter().map(|(k, v)| (k.to_string(), to_proto_value(v))).collect()
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
