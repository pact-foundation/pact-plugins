//! Support for matching and generating content based on content types

use std::str::from_utf8;

use log::error;
use pact_models::bodies::OptionalBody;
use pact_models::matchingrules::MatchingRuleCategory;

use crate::catalogue_manager::{CatalogueEntry, CatalogueEntryProviderType};
use crate::plugin_manager::lookup_plugin;
use crate::proto::{Body, CompareContentsRequest, MatchingRule, MatchingRules};
use crate::utils::to_proto_struct;

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
  pub expected: String,
  /// Actual value in string format
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
    format!("plugin/{}/content-matcher/{}", self.plugin_name(), self.catalogue_entry.key)
  }

  /// Plugin name that provides this matcher
  pub fn plugin_name(&self) -> String {
    self.catalogue_entry.plugin.as_ref()
      .map(|p| p.name.clone())
      .unwrap_or("core".to_string())
  }

  // TODO
  // override fun configureContent(
  // contentType: String,
  // bodyConfig: Map<String, Any?>
  // ): Triple<OptionalBody, MatchingRuleCategory?, Generators?> {
  // logger.debug { "Sending configureContentMatcherInteraction request to for plugin $catalogueEntry" }
  // return DefaultPluginManager.configureContentMatcherInteraction(this, contentType, bodyConfig)
  // }

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
        (k.to_string(), MatchingRules {
          rule: r.rules.iter().map(|rule|{
            MatchingRule {
              r#type: rule.name(),
              values: Some(to_proto_struct(rule.values())),
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

// package io.pact.plugins.jvm.core
//
// import au.com.dius.pact.core.model.ContentType
// import au.com.dius.pact.core.model.OptionalBody
// import au.com.dius.pact.core.model.generators.Generator
// import mu.KLogging
//
// /**
//  * Interface to a content generator
//  */
// interface ContentGenerator {
//   val catalogueEntry: CatalogueEntry
//   /**
//    * If this is a core generator or from a plugin
//    */
//   val isCore: Boolean
//
//   /**
//    * Generate the contents for the body, using the provided generators
//    */
//   fun generateContent(contentType: ContentType, generators: Map<String, Generator>, body: OptionalBody): OptionalBody
// }
//
// open class CatalogueContentGenerator(override val catalogueEntry: CatalogueEntry) : ContentGenerator, KLogging() {
//   override val isCore: Boolean
//     get() = catalogueEntry.providerType == CatalogueEntryProviderType.CORE
//
//   override fun generateContent(
//     contentType: ContentType,
//     generators: Map<String, Generator>,
//     body: OptionalBody
//   ): OptionalBody {
//     return DefaultPluginManager.generateContent(this, contentType, generators, body)
//   }
// }
