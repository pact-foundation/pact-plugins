//! Support for matching and generating content based on content types
use std::collections::HashMap;
use std::str::from_utf8;

use anyhow::anyhow;
use bytes::Bytes;
use log::{debug, error};
use maplit::hashmap;
use pact_models::bodies::OptionalBody;
use pact_models::matchingrules::{Category, MatchingRule, MatchingRuleCategory, RuleList};
use pact_models::path_exp::DocPath;
use pact_models::prelude::{ContentType, Generator, Generators, RuleLogic, GeneratorCategory};
use serde_json::Value;

use crate::catalogue_manager::{CatalogueEntry, CatalogueEntryProviderType};
use crate::plugin_manager::lookup_plugin;
use crate::proto::{Body, CompareContentsRequest, ConfigureContentsRequest, GenerateContentRequest};
use crate::utils::{proto_struct_to_json, to_proto_struct};

/// Matcher for contents based on content type
#[derive(Clone, Debug)]
pub struct ContentMatcher {
  /// Catalogue entry for this content matcher
  pub catalogue_entry: CatalogueEntry
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

  /// Get the plugin to configure the contents for the interaction part based on the provided
  /// definition
  pub async fn configure_content(
    &self,
    content_type: &ContentType,
    definition: HashMap<String, Value>
  ) -> anyhow::Result<(OptionalBody, Option<MatchingRuleCategory>, Option<Generators>)> {
    debug!("Sending ConfigureContents request to plugin {:?}", self.catalogue_entry);
    let request = ConfigureContentsRequest {
      content_type: content_type.to_string(),
      contents_config: Some(to_proto_struct(definition)),
    };

    let plugin_manifest = self.catalogue_entry.plugin.as_ref()
      .expect("Plugin type is required");
    match lookup_plugin(&plugin_manifest.as_dependency()) {
      Some(plugin) => match plugin.configure_contents(request).await {
        Ok(response) => {
          debug!("Got response: {:?}", response);
          let body = match response.contents {
            Some(body) => {
              let returned_content_type = ContentType::parse(body.content_type.as_str()).ok();
              let contents = body.content.unwrap_or_default();
              OptionalBody::Present(Bytes::from(contents), returned_content_type)
            },
            None => OptionalBody::Missing
          };

          let rules = MatchingRuleCategory {
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
              })
            }).collect()
          };

          let generators = Generators {
            categories: hashmap!{
              GeneratorCategory::BODY => response.generators.iter().map(|(k, gen)| {
                // TODO: This is unwrapping the DocPath
                // TODO: This is unwrapping the Generator
                (DocPath::new(k).unwrap(), Generator::create(gen.r#type.as_str(),
                  &gen.values.as_ref().map(|attr| proto_struct_to_json(attr)).unwrap_or_default()).unwrap())
              }).collect()
            }
          };

          debug!("body={}", body);
          debug!("rules={:?}", rules);
          debug!("generators={:?}", generators);

          Ok((body, Some(rules), Some(generators)))
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
    allow_unexpected_keys: bool
  ) -> Result<(), Vec<ContentMismatch>> {
    let request = CompareContentsRequest {
      expected: Some(Body {
        content_type: expected.content_type().unwrap_or_default().to_string(),
        content: expected.value().map(|b| b.to_vec()),
      }),
      actual: Some(Body {
        content_type: actual.content_type().unwrap_or_default().to_string(),
        content: actual.value().map(|b| b.to_vec()),
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
    };

    let plugin_manifest = self.catalogue_entry.plugin.as_ref()
      .expect("Plugin type is required");
    match lookup_plugin(&plugin_manifest.as_dependency()) {
      Some(plugin) => match plugin.compare_contents(request).await {
        Ok(response) => if let Some(mismatch) = response.type_mismatch {
          Err(vec![
            ContentMismatch {
              expected: mismatch.expected.clone(),
              actual: mismatch.actual.clone(),
              mismatch: format!("Expected content type '{}' but got '{}'", mismatch.expected, mismatch.actual),
              path: "".to_string(),
              diff: None
            }
          ])
        } else if !response.results.is_empty() {
          Err(response.results.iter().map(|result| {
            ContentMismatch {
              expected: result.expected.as_ref()
                .map(|e| from_utf8(&e).unwrap_or_default().to_string())
                .unwrap_or_default(),
              actual: result.actual.as_ref()
                .map(|a| from_utf8(&a).unwrap_or_default().to_string())
                .unwrap_or_default(),
              mismatch: result.mismatch.clone(),
              path: result.path.clone(),
              diff: if result.diff.is_empty() {
                None
              } else {
                Some(result.diff.clone())
              }
            }
          }).collect())
        } else {
          Ok(())
        }
        Err(err) => {
          error!("Call to plugin failed - {}", err);
          Err(vec![
            ContentMismatch {
              expected: "".to_string(),
              actual: "".to_string(),
              mismatch: format!("Call to plugin failed = {}", err),
              path: "".to_string(),
              diff: None
            }
          ])
        }
      },
      None => {
        error!("Plugin for {:?} was not found in the plugin register", self.catalogue_entry);
        Err(vec![
          ContentMismatch {
            expected: "".to_string(),
            actual: "".to_string(),
            mismatch: format!("Plugin for {:?} was not found in the plugin register", self.catalogue_entry),
            path: "".to_string(),
            diff: None
          }
        ])
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
    body: &OptionalBody
  ) -> anyhow::Result<OptionalBody> {
    let request = GenerateContentRequest {
      contents: Some(crate::proto::Body {
        content_type: content_type.to_string(),
        content: Some(body.value().unwrap_or_default().to_vec())
      }),
      generators: generators.iter().map(|(k, v)| {
        (k.clone(), crate::proto::Generator {
          r#type: v.name(),
          values: Some(to_proto_struct(v.values().iter()
            .map(|(k, v)| (k.to_string(), v.clone())).collect())),
        })
      }).collect()
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
              ContentType::parse(contents.content_type.as_str()).ok()
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
