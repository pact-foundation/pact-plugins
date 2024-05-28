//! Support for matching and generating content based on content types
use std::collections::HashMap;

use anyhow::anyhow;
use maplit::hashmap;
use pact_models::bodies::OptionalBody;
use pact_models::matchingrules::MatchingRuleCategory;
use pact_models::prelude::{ContentType, Generator, Generators};
use pact_models::plugins::PluginData;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error};

use crate::catalogue_manager::{CatalogueEntry, CatalogueEntryProviderType};
use crate::plugin_manager::lookup_plugin;
use crate::plugin_models::{
  PactPluginManifest,
  PluginInteractionConfig,
  CompareContentRequest,
  CompareContentResult,
  GenerateContentRequest
};
use crate::proto::{PluginConfiguration as ProtoPluginConfiguration};
use crate::utils::proto_struct_to_map;

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
#[derive(Clone, Debug)]
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
  pub metadata: Option<HashMap<String, Value>>,

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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginConfiguration {
  /// Data to persist on the interaction
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
      interaction_configuration: config.interaction_configuration.as_ref().map(|c| proto_struct_to_map(c)).unwrap_or_default(),
      pact_configuration: config.pact_configuration.as_ref().map(|c| proto_struct_to_map(c)).unwrap_or_default()
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
  pub async fn configure_interaction(
    &self,
    content_type: &ContentType,
    definition: HashMap<String, Value>
  ) -> anyhow::Result<(Vec<InteractionContents>, Option<PluginConfiguration>)> {
    debug!("Sending ConfigureContents request to plugin {:?}", self.catalogue_entry);
    let plugin_manifest = self.catalogue_entry.plugin.as_ref()
      .expect("Plugin manifest is required");
    match lookup_plugin(&plugin_manifest.as_dependency()) {
      Some(plugin) => plugin.configure_interaction(content_type, &definition).await,
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
    let request = CompareContentRequest {
      expected_contents: expected.clone(),
      actual_contents: actual.clone(),
      allow_unexpected_keys,
      matching_rules: context.rules.clone(),
      plugin_configuration: plugin_config.clone(),
      .. CompareContentRequest::default()
    };

    let plugin_manifest = self.catalogue_entry.plugin.as_ref()
      .expect("Plugin type is required");
    match lookup_plugin(&plugin_manifest.as_dependency()) {
      Some(plugin) => match plugin.match_contents(request).await {
        Ok(response) => match response {
          CompareContentResult::Error(err) => Err(hashmap! {
              String::default() => vec![
                ContentMismatch {
                  expected: Default::default(),
                  actual: Default::default(),
                  mismatch: err.clone(),
                  path: "".to_string(),
                  diff: None,
                  mismatch_type: None
                }
              ]
            }),
          CompareContentResult::TypeMismatch(expected, actual) => Err(hashmap!{
            String::default() => vec![
              ContentMismatch {
                expected: expected.clone(),
                actual: actual.clone(),
                mismatch: format!("Expected content type '{}' but got '{}'", expected, actual),
                path: "".to_string(),
                diff: None,
                mismatch_type: None
              }
            ]
          }),
          CompareContentResult::Mismatches(mismatches) => {
            if mismatches.is_empty() {
              Ok(())
            } else {
              Err(mismatches.clone())
            }
          }
          CompareContentResult::OK => Ok(())
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
      content_type: content_type.clone(),
      content: body.clone(),
      generators: generators.clone(),
      plugin_data: plugin_data.clone(),
      interaction_data: interaction_data.cloned(),
      test_context: context.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
      .. GenerateContentRequest::default()
    };

    let plugin_manifest = self.catalogue_entry.plugin.as_ref()
      .expect("Plugin type is required");
    match lookup_plugin(&plugin_manifest.as_dependency()) {
      Some(plugin) => {
        debug!("Sending generateContent request to plugin {:?}", plugin_manifest);
        plugin.generate_contents(request).await
      },
      None => {
        error!("Plugin for {:?} was not found in the plugin register", self.catalogue_entry);
        Err(anyhow!("Plugin for {:?} was not found in the plugin register", self.catalogue_entry))
      }
    }
  }
}
