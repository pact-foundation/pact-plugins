//! Support for matching and generating content based on content types
use std::collections::HashMap;
use std::str::from_utf8;

use anyhow::anyhow;
use bytes::Bytes;
use log::{debug, error};
use maplit::hashmap;
use pact_models::bodies::OptionalBody;
use pact_models::content_types::ContentTypeHint;
use pact_models::matchingrules::{Category, MatchingRule, MatchingRuleCategory, RuleList};
use pact_models::path_exp::DocPath;
use pact_models::prelude::{ContentType, Generator, GeneratorCategory, Generators, RuleLogic};
use serde_json::Value;

use crate::catalogue_manager::{CatalogueEntry, CatalogueEntryProviderType};
use crate::plugin_manager::lookup_plugin;
use crate::plugin_models::{PluginInteractionConfig, PactPluginManifest};
use crate::proto::{
  Body,
  CompareContentsRequest,
  ConfigureInteractionRequest,
  GenerateContentRequest,
  PluginConfiguration as ProtoPluginConfiguration
};
use crate::proto::body;
use crate::proto::configure_interaction_response::MarkupType;
use crate::utils::{proto_struct_to_json, proto_struct_to_map, to_proto_struct};

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
  pub diff: Option<String>
}

/// Interaction contents setup by the plugin
#[derive(Clone, Debug)]
pub struct InteractionContents {
  /// Body/Contents of the interaction
  pub body: OptionalBody,
  /// Matching rules to apply
  pub rules: Option<MatchingRuleCategory>,
  /// Generators to apply
  pub generators: Option<Generators>,
  /// Message metadata
  pub metadata: Option<HashMap<String, Value>>,
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
      body: Default::default(),
      rules: None,
      generators: None,
      metadata: None,
      plugin_config: Default::default(),
      interaction_markup: "".to_string(),
      interaction_markup_type: "".to_string()
    }
  }
}

/// Plugin data to persist into the Pact file
#[derive(Clone, Debug)]
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

  /// Get the plugin to configure the interaction contents for the interaction part based on the
  /// provided definition
  pub async fn configure_interation(
    &self,
    content_type: &ContentType,
    definition: HashMap<String, Value>
  ) -> anyhow::Result<InteractionContents> {
    debug!("Sending ConfigureContents request to plugin {:?}", self.catalogue_entry);
    let request = ConfigureInteractionRequest {
      content_type: content_type.to_string(),
      contents_config: Some(to_proto_struct(definition)),
    };

    let plugin_manifest = self.catalogue_entry.plugin.as_ref()
      .expect("Plugin type is required");
    match lookup_plugin(&plugin_manifest.as_dependency()) {
      Some(plugin) => match plugin.configure_interaction(request).await {
        Ok(response) => {
          debug!("Got response: {:?}", response);
          let body = match &response.contents {
            Some(body) => {
              let returned_content_type = ContentType::parse(body.content_type.as_str()).ok();
              let contents = body.content.as_ref().cloned().unwrap_or_default();
              OptionalBody::Present(Bytes::from(contents), returned_content_type,
                  Some(match body.content_type_hint() {
                    body::ContentTypeHint::Text => ContentTypeHint::TEXT,
                    body::ContentTypeHint::Binary => ContentTypeHint::BINARY,
                    body::ContentTypeHint::Default => ContentTypeHint::DEFAULT,
                  }))
            },
            None => OptionalBody::Missing
          };

          let rules = if !response.rules.is_empty() {
            Some(MatchingRuleCategory {
              name: Category::BODY,
              rules: response.rules.iter().map(|(k, rules)| {
                // TODO: This is unwrapping the DocPath
                (DocPath::new(k).unwrap(), RuleList {
                  rules: rules.rule.iter().map(|rule| {
                    // TODO: This is unwrapping the MatchingRule
                    MatchingRule::create(rule.r#type.as_str(), &rule.values.as_ref().map(|rule| {
                      proto_struct_to_json(rule)
                    }).unwrap_or_default()).unwrap()
                  }).collect(),
                  rule_logic: RuleLogic::And,
                  cascaded: false
                })}).collect()
            })
          } else {
            None
          };

          let generators = if !response.generators.is_empty() {
            Some(Generators {
              categories: hashmap! {
                GeneratorCategory::BODY => response.generators.iter().map(|(k, gen)| {
                  // TODO: This is unwrapping the DocPath
                  // TODO: This is unwrapping the Generator
                  (DocPath::new(k).unwrap(), Generator::create(gen.r#type.as_str(),
                    &gen.values.as_ref().map(|attr| proto_struct_to_json(attr)).unwrap_or_default()).unwrap())
                }).collect()
              }
            })
          } else {
            None
          };

          let metadata = response.message_metadata.as_ref().map(|md| proto_struct_to_map(md));

          let plugin_config = if let Some(plugin_configuration) = &response.plugin_configuration {
            PluginConfiguration {
              interaction_configuration: plugin_configuration.interaction_configuration.as_ref()
                .map(|val| proto_struct_to_map(val)).unwrap_or_default(),
              pact_configuration: plugin_configuration.pact_configuration.as_ref()
                .map(|val| proto_struct_to_map(val)).unwrap_or_default()
            }
          } else {
            PluginConfiguration::default()
          };

          debug!("body={}", body);
          debug!("rules={:?}", rules);
          debug!("generators={:?}", generators);
          debug!("metadata={:?}", metadata);
          debug!("pluginConfig={:?}", plugin_config);

          Ok(InteractionContents {
            body,
            rules,
            generators,
            metadata,
            plugin_config,
            interaction_markup: response.interaction_markup.clone(),
            interaction_markup_type: match response.interaction_markup_type() {
              MarkupType::Html => "HTML".to_string(),
              _ => "COMMON_MARK".to_string(),
            }
          })
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
              values: Some(to_proto_struct(rule.values().iter().map(|(k, v)| (k.to_string(), v.clone())).collect())),
            }
          }).collect()
        })
      }).collect(),
      plugin_configuration: plugin_config.map(|config| ProtoPluginConfiguration {
        interaction_configuration: Some(to_proto_struct(config.interaction_configuration)),
        pact_configuration: Some(to_proto_struct(config.pact_configuration))
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
                diff: None
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
                diff: None
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
                }
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
                diff: None
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
              diff: None
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
  // TODO need to pass in any plugin configuration
  pub async fn generate_content(
    &self,
    content_type: &ContentType,
    generators: &HashMap<String, Generator>,
    body: &OptionalBody
  ) -> anyhow::Result<OptionalBody> {
    let request = GenerateContentRequest {
      contents: Some(crate::proto::Body {
        content_type: content_type.to_string(),
        content: Some(body.value().unwrap_or_default().to_vec()),
        content_type_hint: body::ContentTypeHint::Default as i32
      }),
      generators: generators.iter().map(|(k, v)| {
        (k.clone(), crate::proto::Generator {
          r#type: v.name(),
          values: Some(to_proto_struct(v.values().iter()
            .map(|(k, v)| (k.to_string(), v.clone())).collect())),
        })
      }).collect(),
      plugin_configuration: None
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
