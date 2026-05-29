use prost_types::{ListValue, Struct, Value as ProtoValue};
use serde_json::{Map, Number, Value};

tonic::include_proto!("io.pact.plugin.v2");

pub fn json_to_proto_struct(value: &Value) -> Struct {
  match value {
    Value::Object(fields) => Struct {
      fields: fields
        .iter()
        .map(|(key, value)| (key.clone(), json_to_proto_value(value)))
        .collect(),
    },
    _ => Struct::default(),
  }
}

pub fn json_to_proto_value(value: &Value) -> ProtoValue {
  let kind = match value {
    Value::Null => prost_types::value::Kind::NullValue(0),
    Value::Bool(value) => prost_types::value::Kind::BoolValue(*value),
    Value::Number(value) => {
      prost_types::value::Kind::NumberValue(value.as_f64().unwrap_or_default())
    }
    Value::String(value) => prost_types::value::Kind::StringValue(value.clone()),
    Value::Array(values) => prost_types::value::Kind::ListValue(ListValue {
      values: values.iter().map(json_to_proto_value).collect(),
    }),
    Value::Object(fields) => {
      prost_types::value::Kind::StructValue(json_to_proto_struct(&Value::Object(fields.clone())))
    }
  };

  ProtoValue { kind: Some(kind) }
}

pub fn proto_struct_to_json(value: &Struct) -> Value {
  Value::Object(
    value
      .fields
      .iter()
      .map(|(key, value)| (key.clone(), proto_value_to_json(value)))
      .collect::<Map<String, Value>>(),
  )
}

pub fn proto_value_to_json(value: &ProtoValue) -> Value {
  match value.kind.as_ref() {
    Some(prost_types::value::Kind::NullValue(_)) | None => Value::Null,
    Some(prost_types::value::Kind::BoolValue(value)) => Value::Bool(*value),
    Some(prost_types::value::Kind::NumberValue(value)) => {
      if value.is_finite()
        && value.fract() == 0.0
        && *value >= i64::MIN as f64
        && *value <= i64::MAX as f64
      {
        Value::Number(Number::from(*value as i64))
      } else {
        Number::from_f64(*value)
          .map(Value::Number)
          .unwrap_or(Value::Null)
      }
    }
    Some(prost_types::value::Kind::StringValue(value)) => Value::String(value.clone()),
    Some(prost_types::value::Kind::ListValue(values)) => {
      Value::Array(values.values.iter().map(proto_value_to_json).collect())
    }
    Some(prost_types::value::Kind::StructValue(value)) => proto_struct_to_json(value),
  }
}

pub fn string_metadata(value: impl Into<String>) -> MetadataValue {
  MetadataValue {
    value: Some(metadata_value::Value::NonBinaryValue(ProtoValue {
      kind: Some(prost_types::value::Kind::StringValue(value.into())),
    })),
  }
}

pub fn json_body(content: Vec<u8>) -> Body {
  Body {
    content_type: "application/json".to_string(),
    content: Some(content),
    content_type_hint: body::ContentTypeHint::Text as i32,
  }
}
