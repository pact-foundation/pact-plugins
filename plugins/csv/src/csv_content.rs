use anyhow::anyhow;
use bytes::Bytes;
use csv::{ReaderBuilder, Writer};
use either::Either;
use either::Either::{Left, Right};
use itertools::Itertools;
use log::debug;
use maplit::hashmap;
use pact_models::bodies::OptionalBody;
use pact_models::generators::{GenerateValue, Generator, NoopVariantMatcher, VariantMatcher};
use pact_models::prelude::ContentType;
use serde_json::{json, Value};
use tonic::{Request, Response};

use pact_plugin_driver::utils::{to_proto_struct, proto_struct_to_map};

use crate::parser::{parse_field, parse_value};
use crate::proto;
use crate::utils::{from_value, to_boolean, to_value};

pub fn setup_csv_contents(
  request: &Request<proto::ConfigureInteractionRequest>
) -> anyhow::Result<Response<proto::ConfigureInteractionResponse>> {
  match &request.get_ref().contents_config {
    Some(config) => {
      let mut columns = vec![];
      let has_headers = config.fields.get("csvHeaders").map(|val| to_boolean(val)).unwrap_or(true);

      for (key, value) in &config.fields {
        if key.starts_with("column:") {
          let column = parse_field(&key)?;
          let result = parse_value(&value)?;
          debug!("Parsed column definition: {}, {:?}", column, result);
          match column {
            Either::Left(i) => {
              if i > columns.len() {
                columns.resize(i, None)
              }
              columns[i - 1] = Some((result, i.to_string()));
            }
            Either::Right(s) => {
              columns.push(Some((result, s.clone())))
            }
          }
        }
      }

      let mut wtr = Writer::from_writer(vec![]);
      let mut csv_markup = String::new();

      csv_markup.push_str("# Data\n\n");
      if has_headers {
        let column_values = columns.iter().map(|v| {
          if let Some((_, name)) = v {
            name.as_str()
          } else {
            ""
          }

        }).collect::<Vec<&str>>();

        wtr.write_record(column_values.clone())?;

        csv_markup.push('|');
        csv_markup.push_str(column_values.iter().join("|").as_str());
        csv_markup.push_str("|\n|");
        csv_markup.push_str(column_values.iter().map(|col| {
          let mut s = String::new();
          for _ in 1..(col.len()) {
            s.push('-');
          }
          s
        }).join("|").as_str());
        csv_markup.push_str("|\n");
      }

      let column_values = columns.iter().map(|v| {
        if let Some((md, _)) = v {
          md.value.as_str()
        } else {
          ""
        }
      }).collect::<Vec<&str>>();
      wtr.write_record(column_values.clone())?;

      csv_markup.push('|');
      csv_markup.push_str(column_values.iter().join("|").as_str());
      csv_markup.push_str("|\n");

      let mut rules = hashmap!{};
      let mut generators = hashmap!{};
      for vals in columns {
        if let Some((md, name)) = vals {
          for rule in md.rules {
            if let Either::Left(rule) = rule {
              debug!("rule.values()={:?}", rule.values());
              rules.insert(format!("column:{}", name), proto::MatchingRules {
                rule: vec![
                  proto::MatchingRule {
                    r#type: rule.name(),
                    values: Some(prost_types::Struct {
                      fields: rule.values().iter().map(|(key, val)| (key.to_string(), to_value(val))).collect()
                    })
                  }
                ]
              });
            } else {
              return Ok(Response::new(proto::ConfigureInteractionResponse {
                error: format!("Expected a matching rule definition, but got an un-resolved reference {:?}", rule),
                .. proto::ConfigureInteractionResponse::default()
              }));
            }
          }

          if let Some(gen) = md.generator {
            generators.insert(format!("column:{}", name), proto::Generator {
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

      Ok(Response::new(proto::ConfigureInteractionResponse {
        interaction: vec![proto::InteractionResponse {
          contents: Some(proto::Body {
            content_type: "text/csv;charset=UTF-8".to_string(),
            content: Some(wtr.into_inner()?),
            content_type_hint: 0
          }),
          rules,
          generators,
          message_metadata: None,
          plugin_configuration: Some(proto::PluginConfiguration {
            interaction_configuration: Some(to_proto_struct(&hashmap!{
            "csvHeaders".to_string() => json!(has_headers)
          })),
            pact_configuration: None
          }),
          interaction_markup: csv_markup,
          interaction_markup_type: 0,
          .. proto::InteractionResponse::default()
        }],
        .. proto::ConfigureInteractionResponse::default()
      }))
    }
    None => Err(anyhow!("No config provided to match/generate CSV content"))
  }
}

pub fn generate_csv_content(
  request: &Request<proto::GenerateContentRequest>
) -> anyhow::Result<OptionalBody> {
  let request = request.get_ref();
  let has_headers = has_headers(&request.plugin_configuration);

  let mut generators = hashmap! {};
  for (key, gen) in &request.generators {
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

  let csv_data = request.contents.as_ref().unwrap().content.as_ref().unwrap();
  let mut rdr = ReaderBuilder::new().has_headers(has_headers).from_reader(csv_data.as_slice());
  let headers = rdr.headers()?.clone();

  if has_headers {
    wtr.write_record(&headers)?;
  }

  for result in rdr.records() {
    let record = result?;
    for (col, field) in record.iter().enumerate() {
      debug!("got column:{} = '{}'", col, field);
      if has_headers {
        if let Some(generator) = generators.get(&Right(headers.get(col).unwrap_or_default().to_string())) {
          let value = generator.generate_value(&field.to_string(), &context, &variant_matcher)?;
          wtr.write_field(value)?;
        } else if let Some(generator) = generators.get(&Left(col)) {
          let value = generator.generate_value(&field.to_string(), &context, &variant_matcher)?;
          wtr.write_field(value)?;
        } else {
          wtr.write_field(field)?;
        }
      } else {
        if let Some(generator) = generators.get(&Left(col)) {
          let value = generator.generate_value(&field.to_string(), &context, &variant_matcher)?;
          wtr.write_field(value)?;
        } else {
          wtr.write_field(field)?;
        }
      }
    }
    wtr.write_record(None::<&[u8]>)?;
  }
  let generated = wtr.into_inner()?;
  debug!("Generated contents has {} bytes", generated.len());
  let bytes = Bytes::from(generated);
  Ok(OptionalBody::Present(bytes, Some(ContentType::from("text/csv;charset=UTF-8")), None))
}

pub fn has_headers(plugin_config: &Option<proto::PluginConfiguration>) -> bool {
  match &plugin_config {
    Some(config) => match &config.interaction_configuration {
      Some(i_config) => proto_struct_to_map(&i_config).get("csvHeaders").map(|val| match val {
        Value::Bool(b) => *b,
        _ => true
      }).unwrap_or(true),
      None => true
    }
    None => true
  }
}
