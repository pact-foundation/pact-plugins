//! Support for matching and generating content based on content types
use std::collections::{BTreeMap, HashMap};
use std::str::from_utf8;

use anyhow::anyhow;
use bytes::Bytes;
use maplit::hashmap;
use pact_models::bodies::OptionalBody;
use pact_models::content_types::ContentTypeHint;
use pact_models::matchingrules::{Category, MatchingRule, MatchingRuleCategory, RuleList};
use pact_models::path_exp::DocPath;
use pact_models::plugins::PluginData;
use pact_models::prelude::{ContentType, Generator, GeneratorCategory, Generators, RuleLogic};
use serde_json::Value;
use tracing::{debug, error};

use crate::catalogue_manager::{CatalogueEntry, CatalogueEntryProviderType};
use crate::plugin_manager::lookup_plugin;
use crate::plugin_models::{PactPluginManifest, PactPluginRpc, PluginInteractionConfig};
use crate::proto::{
  Body,
  CompareContentsRequest,
  ConfigureInteractionRequest,
  ConfigureInteractionResponse,
  GenerateContentRequest,
  PluginConfiguration as ProtoPluginConfiguration
};
use crate::proto::body;
use crate::proto::interaction_response::MarkupType;
use crate::utils::{proto_struct_to_hashmap, proto_struct_to_json, proto_struct_to_map, to_proto_struct};

/// Matcher for contents based on content type
#[derive(Clone, Debug)]
pub struct ContentMatcher {
  /// Catalogue entry for this content matcher
  pub catalogue_entry: CatalogueEntry
}

impl ContentMatcher {
  /// Plugin details for this content matcher
  pub fn plugin(&self) -> Option<PactPluginManifest> {
    self.catalogue_entry.plugin.clone()
  }
}

/// Mismatch result
#[derive(Clone, Debug)]
pub struct ContentMismatch {
  /// Expected value in string format
  // TODO: change to bytes
  pub expected: String,
  /// Actual value in string format
  // TODO: change to bytes
  pub actual: String,
  /// Mismatch description
  pub mismatch: String,
  /// Path to the mismatch
  pub path: String,
  /// Optional diff of the expected and actual values
  pub diff: Option<String>,
  /// The type of item that the mismatch is for
  pub mismatch_type: Option<String>
}

/// Interaction contents setup by the plugin
#[derive(Clone, Debug, PartialEq)]
pub struct InteractionContents {
  /// Description of what part this interaction belongs to (in the case of there being more than
  /// one, for instance, request/response messages)
  pub part_name: String,

  /// Body/Contents of the interaction
  pub body: OptionalBody,

  /// Matching rules to apply
  pub rules: Option<MatchingRuleCategory>,

  /// Generators to apply
  pub generators: Option<Generators>,

  /// Message metadata
  pub metadata: Option<BTreeMap<String, Value>>,

  /// Matching rules to apply to message metadata
  pub metadata_rules: Option<MatchingRuleCategory>,

  /// Plugin configuration data to apply to the interaction
  pub plugin_config: PluginConfiguration,

  /// Markup for the interaction to display in any UI
  pub interaction_markup: String,

  /// The type of the markup (CommonMark or HTML)
  pub interaction_markup_type: String
}

impl Default for InteractionContents {
  fn default() -> Self {
    InteractionContents {
      part_name: Default::default(),
      body: Default::default(),
      rules: None,
      generators: None,
      metadata: None,
      metadata_rules: None,
      plugin_config: Default::default(),
      interaction_markup: Default::default(),
      interaction_markup_type: Default::default()
    }
  }
}

/// Plugin data to persist into the Pact file
#[derive(Clone, Debug, PartialEq)]
pub struct PluginConfiguration {
  /// Data to perist on the interaction
  pub interaction_configuration: HashMap<String, Value>,
  /// Data to persist in the Pact metadata
  pub pact_configuration: HashMap<String, Value>
}

impl PluginConfiguration {
  /// Plugin data is empty when the interaction and pact data is empty
  pub fn is_empty(&self) -> bool {
    self.pact_configuration.is_empty() && self.interaction_configuration.is_empty()
  }
}

impl Default for PluginConfiguration {
  fn default() -> Self {
    PluginConfiguration {
      interaction_configuration: Default::default(),
      pact_configuration: Default::default()
    }
  }
}

impl From<ProtoPluginConfiguration> for PluginConfiguration {
  fn from(config: ProtoPluginConfiguration) -> Self {
    PluginConfiguration {
      interaction_configuration: config.interaction_configuration
        .as_ref()
        .map(|c| proto_struct_to_hashmap(c))
        .unwrap_or_default(),
      pact_configuration: config.pact_configuration
        .as_ref()
        .map(|c| proto_struct_to_hashmap(c))
        .unwrap_or_default()
    }
  }
}

impl ContentMatcher {
  /// If this is a core framework matcher
  pub fn is_core(&self) -> bool {
    self.catalogue_entry.provider_type == CatalogueEntryProviderType::CORE
  }

  /// Catalogue entry key for this matcher
  pub fn catalogue_entry_key(&self) -> String {
    if self.is_core() {
      format!("core/content-matcher/{}", self.catalogue_entry.key)
    } else {
      format!("plugin/{}/content-matcher/{}", self.plugin_name(), self.catalogue_entry.key)
    }
  }

  /// Plugin name that provides this matcher
  pub fn plugin_name(&self) -> String {
    self.catalogue_entry.plugin.as_ref()
      .map(|p| p.name.clone())
      .unwrap_or("core".to_string())
  }

  /// Plugin version that provides this matcher
  pub fn plugin_version(&self) -> String {
    self.catalogue_entry.plugin.as_ref()
      .map(|p| p.version.clone())
      .unwrap_or_default()
  }

  /// Get the plugin to configure the interaction contents for the interaction part based on the
  /// provided definition
  #[deprecated(note = "Use the version that is spelled correctly")]
  pub async fn configure_interation(
    &self,
    content_type: &ContentType,
    definition: HashMap<String, Value>
  ) -> anyhow::Result<(Vec<InteractionContents>, Option<PluginConfiguration>)> {
    self.configure_interaction(content_type, definition).await
  }

  /// Get the plugin to configure the interaction contents for the interaction part based on the
  /// provided definition
  pub async fn configure_interaction(
    &self,
    content_type: &ContentType,
    definition: HashMap<String, Value>
  ) -> anyhow::Result<(Vec<InteractionContents>, Option<PluginConfiguration>)> {
    debug!("Sending ConfigureContents request to plugin {:?}", self.catalogue_entry);
    let request = ConfigureInteractionRequest {
      content_type: content_type.to_string(),
      contents_config: Some(to_proto_struct(&definition)),
    };

    let plugin_manifest = self.catalogue_entry.plugin.as_ref()
      .expect("Plugin type is required");
    match lookup_plugin(&plugin_manifest.as_dependency()) {
      Some(plugin) => match plugin.configure_interaction(request).await {
        Ok(response) => {
          debug!("Got response: {:?}", response);
          if response.error.is_empty() {
            let results = Self::build_interaction_contents(&response)?;
            Ok((results, response.plugin_configuration.map(|config| PluginConfiguration::from(config))))
          } else {
            Err(anyhow!("Request to configure interaction failed: {}", response.error))
          }
        }
        Err(err) => {
          error!("Call to plugin failed - {}", err);
          Err(anyhow!("Call to plugin failed - {}", err))
        }
      },
      None => {
        error!("Plugin for {:?} was not found in the plugin register", self.catalogue_entry);
        Err(anyhow!("Plugin for {:?} was not found in the plugin register", self.catalogue_entry))
      }
    }
  }

  pub(crate) fn build_interaction_contents(
    response: &ConfigureInteractionResponse
  ) -> anyhow::Result<Vec<InteractionContents>> {
    let mut results = vec![];

    for response in &response.interaction {
      let body = match &response.contents {
        Some(body) => {
          let contents = body.content.as_ref().cloned().unwrap_or_default();
          if contents.is_empty() {
            OptionalBody::Empty
          } else {
            let returned_content_type = ContentType::parse(body.content_type.as_str()).ok();
            OptionalBody::Present(Bytes::from(contents), returned_content_type,
                                  Some(match body.content_type_hint() {
                                    body::ContentTypeHint::Text => ContentTypeHint::TEXT,
                                    body::ContentTypeHint::Binary => ContentTypeHint::BINARY,
                                    body::ContentTypeHint::Default => ContentTypeHint::DEFAULT,
                                  }))
          }
        },
        None => OptionalBody::Missing
      };

      let rules = Self::setup_matching_rules(&response.rules)?;

      let generators = if !response.generators.is_empty() || !response.metadata_generators.is_empty() {
        let mut categories = hashmap! {};

        if !response.generators.is_empty() {
          let mut generators = hashmap! {};
          for (k, g) in &response.generators {
            generators.insert(DocPath::new(k)?,
                              Generator::create(g.r#type.as_str(),
                                                &g.values.as_ref().map(|attr| proto_struct_to_json(attr)).unwrap_or_default())?);
          }
          categories.insert(GeneratorCategory::BODY, generators);
        }

        if !response.metadata_generators.is_empty() {
          let mut generators = hashmap! {};
          for (k, g) in &response.metadata_generators {
            generators.insert(DocPath::new(k)?,
                              Generator::create(g.r#type.as_str(),
                                                &g.values.as_ref().map(|attr| proto_struct_to_json(attr)).unwrap_or_default())?);
          }
          categories.insert(GeneratorCategory::METADATA, generators);
        }

        Some(Generators { categories })
      } else {
        None
      };

      let metadata = response.message_metadata.as_ref().map(|md| proto_struct_to_map(md));
      let metadata_rules = Self::setup_matching_rules(&response.metadata_rules)?;

      let plugin_config = if let Some(plugin_configuration) = &response.plugin_configuration {
        PluginConfiguration {
          interaction_configuration: plugin_configuration.interaction_configuration.as_ref()
            .map(|val| proto_struct_to_hashmap(val)).unwrap_or_default(),
          pact_configuration: plugin_configuration.pact_configuration.as_ref()
            .map(|val| proto_struct_to_hashmap(val)).unwrap_or_default()
        }
      } else {
        PluginConfiguration::default()
      };

      debug!("body={}", body);
      debug!("rules={:?}", rules);
      debug!("generators={:?}", generators);
      debug!("metadata={:?}", metadata);
      debug!("metadata_rules={:?}", metadata_rules);
      debug!("pluginConfig={:?}", plugin_config);

      results.push(InteractionContents {
        part_name: response.part_name.clone(),
        body,
        rules,
        generators,
        metadata,
        metadata_rules,
        plugin_config,
        interaction_markup: response.interaction_markup.clone(),
        interaction_markup_type: match response.interaction_markup_type() {
          MarkupType::Html => "HTML".to_string(),
          _ => "COMMON_MARK".to_string(),
        }
      })
    }

    Ok(results)
  }

  fn setup_matching_rules(rules_map: &HashMap<String, crate::proto::MatchingRules>) -> anyhow::Result<Option<MatchingRuleCategory>> {
    if !rules_map.is_empty() {
      let mut rules = hashmap!{};
      for (k, rule_list) in rules_map {
        let mut vec = vec![];
        for rule in &rule_list.rule {
          let mr = MatchingRule::create(rule.r#type.as_str(), &rule.values.as_ref().map(|rule| {
            proto_struct_to_json(rule)
          }).unwrap_or_default())?;
          vec.push(mr);
        }
        rules.insert(DocPath::new(k)?, RuleList {
          rules: vec,
          rule_logic: RuleLogic::And,
          cascaded: false
        });
      }
      Ok(Some(MatchingRuleCategory { name: Category::BODY, rules }))
    } else {
      Ok(None)
    }
  }

  /// Get the plugin to match the contents against the expected contents returning all the mismatches.
  /// Note that it is an error to call this with a non-plugin (core) content matcher.
  ///
  /// panics:
  /// If called with a core content matcher
  pub async fn match_contents(
    &self,
    expected: &OptionalBody,
    actual: &OptionalBody,
    context: &MatchingRuleCategory,
    allow_unexpected_keys: bool,
    plugin_config: Option<PluginInteractionConfig>
  ) -> Result<(), HashMap<String, Vec<ContentMismatch>>> {
    let request = CompareContentsRequest {
      expected: Some(Body {
        content_type: expected.content_type().unwrap_or_default().to_string(),
        content: expected.value().map(|b| b.to_vec()),
        content_type_hint: body::ContentTypeHint::Default as i32
      }),
      actual: Some(Body {
        content_type: actual.content_type().unwrap_or_default().to_string(),
        content: actual.value().map(|b| b.to_vec()),
        content_type_hint: body::ContentTypeHint::Default as i32
      }),
      allow_unexpected_keys,
      rules: context.rules.iter().map(|(k, r)| {
        (k.to_string(), crate::proto::MatchingRules {
          rule: r.rules.iter().map(|rule|{
            crate::proto::MatchingRule {
              r#type: rule.name(),
              values: Some(to_proto_struct(&rule.values().iter().map(|(k, v)| (k.to_string(), v.clone())).collect())),
            }
          }).collect()
        })
      }).collect(),
      plugin_configuration: plugin_config.map(|config| ProtoPluginConfiguration {
        interaction_configuration: Some(to_proto_struct(&config.interaction_configuration)),
        pact_configuration: Some(to_proto_struct(&config.pact_configuration))
      })
    };

    let plugin_manifest = self.catalogue_entry.plugin.as_ref()
      .expect("Plugin type is required");
    match lookup_plugin(&plugin_manifest.as_dependency()) {
      Some(plugin) => match plugin.compare_contents(request).await {
        Ok(response) => if let Some(mismatch) = response.type_mismatch {
          Err(hashmap!{
            String::default() => vec![
              ContentMismatch {
                expected: mismatch.expected.clone(),
                actual: mismatch.actual.clone(),
                mismatch: format!("Expected content type '{}' but got '{}'", mismatch.expected, mismatch.actual),
                path: "".to_string(),
                diff: None,
                mismatch_type: None
              }
            ]
          })
        } else if !response.error.is_empty() {
          Err(hashmap! {
            String::default() => vec![
              ContentMismatch {
                expected: Default::default(),
                actual: Default::default(),
                mismatch: response.error.clone(),
                path: "".to_string(),
                diff: None,
                mismatch_type: None
              }
            ]
          })
        } else if !response.results.is_empty() {
          Err(response.results.iter().map(|(k, v)| {
            (k.clone(), v.mismatches.iter().map(|mismatch| {
              ContentMismatch {
                expected: mismatch.expected.as_ref()
                  .map(|e| from_utf8(&e).unwrap_or_default().to_string())
                  .unwrap_or_default(),
                actual: mismatch.actual.as_ref()
                  .map(|a| from_utf8(&a).unwrap_or_default().to_string())
                  .unwrap_or_default(),
                mismatch: mismatch.mismatch.clone(),
                path: mismatch.path.clone(),
                diff: if mismatch.diff.is_empty() {
                  None
                } else {
                  Some(mismatch.diff.clone())
                },
                mismatch_type: Some(mismatch.mismatch_type.clone())
              }
            }).collect())
          }).collect())
        } else {
          Ok(())
        }
        Err(err) => {
          error!("Call to plugin failed - {}", err);
          Err(hashmap! {
            String::default() => vec![
              ContentMismatch {
                expected: "".to_string(),
                actual: "".to_string(),
                mismatch: format!("Call to plugin failed = {}", err),
                path: "".to_string(),
                diff: None,
                mismatch_type: None
              }
            ]
          })
        }
      },
      None => {
        error!("Plugin for {:?} was not found in the plugin register", self.catalogue_entry);
        Err(hashmap! {
          String::default() => vec![
            ContentMismatch {
              expected: "".to_string(),
              actual: "".to_string(),
              mismatch: format!("Plugin for {:?} was not found in the plugin register", self.catalogue_entry),
              path: "".to_string(),
              diff: None,
              mismatch_type: None
            }
          ]
        })
      }
    }
  }
}

/// Generator for contents based on content type
#[derive(Clone, Debug)]
pub struct ContentGenerator {
  /// Catalogue entry for this content matcher
  pub catalogue_entry: CatalogueEntry
}

impl ContentGenerator {
  /// If this is a core framework generator
  pub fn is_core(&self) -> bool {
    self.catalogue_entry.provider_type == CatalogueEntryProviderType::CORE
  }

  /// Catalogue entry key for this generator
  pub fn catalogue_entry_key(&self) -> String {
    if self.is_core() {
      format!("core/content-generator/{}", self.catalogue_entry.key)
    } else {
      format!("plugin/{}/content-generator/{}", self.plugin_name(), self.catalogue_entry.key)
    }
  }

  /// Plugin name that provides this matcher
  pub fn plugin_name(&self) -> String {
    self.catalogue_entry.plugin.as_ref()
      .map(|p| p.name.clone())
      .unwrap_or("core".to_string())
  }

  /// Generate the content for the given content type and body
  pub async fn generate_content(
    &self,
    content_type: &ContentType,
    generators: &HashMap<String, Generator>,
    body: &OptionalBody,
    plugin_data: &Vec<PluginData>,
    interaction_data: &HashMap<String, HashMap<String, Value>>,
    context: &HashMap<&str, Value>
  ) -> anyhow::Result<OptionalBody> {
    let pact_plugin_manifest = self.catalogue_entry.plugin.clone().unwrap_or_default();
    let plugin_data = plugin_data.iter().find_map(|pd| {
      if pact_plugin_manifest.name == pd.name {
        Some(pd.configuration.clone())
      } else {
        None
      }
    });
    let interaction_data = interaction_data.get(&pact_plugin_manifest.name);

    let request = GenerateContentRequest {
      contents: Some(crate::proto::Body {
        content_type: content_type.to_string(),
        content: Some(body.value().unwrap_or_default().to_vec()),
        content_type_hint: body::ContentTypeHint::Default as i32
      }),
      generators: generators.iter().map(|(k, v)| {
        (k.clone(), crate::proto::Generator {
          r#type: v.name(),
          values: Some(to_proto_struct(&v.values().iter()
            .map(|(k, v)| (k.to_string(), v.clone())).collect())),
        })
      }).collect(),
      plugin_configuration: Some(ProtoPluginConfiguration {
        pact_configuration: plugin_data.as_ref().map(to_proto_struct),
        interaction_configuration: interaction_data.map(to_proto_struct),
        .. ProtoPluginConfiguration::default()
      }),
      test_context: Some(to_proto_struct(&context.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())),
      .. GenerateContentRequest::default()
    };

    let plugin_manifest = self.catalogue_entry.plugin.as_ref()
      .expect("Plugin type is required");
    match lookup_plugin(&plugin_manifest.as_dependency()) {
      Some(plugin) => {
        debug!("Sending generateContent request to plugin {:?}", plugin_manifest);
        match plugin.generate_content(request).await?.contents {
          Some(contents) => {
            Ok(OptionalBody::Present(
              Bytes::from(contents.content.unwrap_or_default()),
              ContentType::parse(contents.content_type.as_str()).ok(),
              None
            ))
          }
          None => Ok(OptionalBody::Empty)
        }
      },
      None => {
        error!("Plugin for {:?} was not found in the plugin register", self.catalogue_entry);
        Err(anyhow!("Plugin for {:?} was not found in the plugin register", self.catalogue_entry))
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use bytes::Bytes;
  use maplit::btreemap;
  use pact_models::bodies::OptionalBody;
  use pact_models::content_types::{ContentType, ContentTypeHint};
  use pretty_assertions::assert_eq;
  use prost_types::value::Kind::StringValue;
  use serde_json::Value;

  use crate::proto::{Body, body, ConfigureInteractionResponse, InteractionResponse};

  use super::{ContentMatcher, InteractionContents};

  // Issue https://github.com/YOU54F/pact-ruby-ffi/issues/6
  #[test_log::test]
  fn build_interaction_contents_deals_with_empty_contents() {
    let response = ConfigureInteractionResponse {
      interaction: vec![
        InteractionResponse {
          contents: Some(Body {
            content_type: "application/protobuf; message=.area_calculator.ShapeMessage".to_string(),
            content: Some(b"\x12\n\r\0\0@@\x15\0\0\x80@".to_vec()),
            content_type_hint: body::ContentTypeHint::Binary.into()
          }),
          message_metadata: Some(prost_types::Struct {
            fields: btreemap!{
              "contentType".to_string() => prost_types::Value {
                kind: Some(StringValue("application/protobuf;message=.area_calculator.ShapeMessage".to_string()))
              }
            }
          }),
          part_name: "request".to_string(),
          .. InteractionResponse::default()
        },
        InteractionResponse {
          contents: Some(Body {
            content_type: "application/protobuf; message=.area_calculator.AreaResponse".to_string(),
            content: Some(vec![]),
            content_type_hint: body::ContentTypeHint::Binary.into()
          }),
          message_metadata: Some(prost_types::Struct {
            fields: btreemap!{
              "grpc-message".to_string() => prost_types::Value {
                kind: Some(StringValue("Not implemented".to_string()))
              },
              "grpc-status".to_string() => prost_types::Value {
                kind: Some(StringValue("UNIMPLEMENTED".to_string()))
              },
              "contentType".to_string() => prost_types::Value {
                kind: Some(StringValue("application/protobuf;message=.area_calculator.AreaResponse".to_string()))
              }
            }
          }),
          part_name: "response".to_string(),
          .. InteractionResponse::default()
        }
      ],
      .. ConfigureInteractionResponse::default()
    };
    let result = ContentMatcher::build_interaction_contents(&response).unwrap();

    assert_eq!(result, vec![
      InteractionContents {
        part_name: "request".to_string(),
        interaction_markup_type: "COMMON_MARK".to_string(),
        body: OptionalBody::Present(Bytes::from(b"\x12\n\r\0\0@@\x15\0\0\x80@".to_vec()),
          Some(ContentType::parse("application/protobuf;message=.area_calculator.ShapeMessage").unwrap()),
          Some(ContentTypeHint::BINARY)),
        metadata: Some(btreemap!{
          "contentType".to_string() => Value::String("application/protobuf;message=.area_calculator.ShapeMessage".to_string())
        }),
        .. InteractionContents::default()
      },

      InteractionContents {
        part_name: "response".to_string(),
        interaction_markup_type: "COMMON_MARK".to_string(),
        body: OptionalBody::Empty,
        metadata: Some(btreemap!{
          "grpc-status".to_string() => Value::String("UNIMPLEMENTED".to_string()),
          "grpc-message".to_string() => Value::String("Not implemented".to_string()),
          "contentType".to_string() => Value::String("application/protobuf;message=.area_calculator.AreaResponse".to_string())
        }),
        .. InteractionContents::default()
      }
    ]);
  }
}
