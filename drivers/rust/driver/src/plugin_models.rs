//! Models for representing plugins

use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;

use async_trait::async_trait;
use pact_models::bodies::OptionalBody;
use pact_models::content_types::ContentType;
use pact_models::generators::Generator;
use pact_models::matchingrules::RuleList;
use pact_models::pact::Pact;
use pact_models::path_exp::DocPath;
use pact_models::prelude::v4::V4Pact;
use pact_models::v4::interaction::V4Interaction;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::catalogue_manager::CatalogueEntry;
use crate::content::{ContentMismatch, InteractionContents, PluginConfiguration};
use crate::mock_server::{MockServerConfig, MockServerDetails, MockServerResults};
use crate::verification::{InteractionVerificationData, InteractionVerificationResult};

/// Type of plugin dependencies
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub enum PluginDependencyType {
  /// Required operating system package
  OSPackage,
  /// Dependency on another plugin
  Plugin,
  /// Dependency on a shared library
  Library,
  /// Dependency on an executable
  Executable
}

impl Default for PluginDependencyType {
  fn default() -> Self {
    PluginDependencyType::Plugin
  }
}

/// Plugin dependency
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PluginDependency {
  /// Dependency name
  pub name: String,
  /// Dependency version (semver format)
  pub version: Option<String>,
  /// Type of dependency
  #[serde(default)]
  pub dependency_type: PluginDependencyType
}

impl Display for PluginDependency {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    if let Some(version) = &self.version {
      write!(f, "{}:{}", self.name, version)
    } else {
      write!(f, "{}:*", self.name)
    }
  }
}

/// Manifest of a plugin
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PactPluginManifest {
  /// Directory were the plugin was loaded from
  #[serde(skip)]
  pub plugin_dir: String,

  /// Interface version supported by the plugin
  pub plugin_interface_version: u8,

  /// Plugin name
  pub name: String,

  /// Plugin version in semver format
  pub version: String,

  /// Type if executable of the plugin
  pub executable_type: String,

  /// Minimum required version for the executable type
  pub minimum_required_version: Option<String>,

  /// How to invoke the plugin
  pub entry_point: String,

  /// Additional entry points for other operating systems (i.e. requiring a .bat file for Windows)
  #[serde(default)]
  pub entry_points: HashMap<String, String>,

  /// Parameters to pass into the command line
  pub args: Option<Vec<String>>,

  /// Dependencies required to invoke the plugin
  pub dependencies: Option<Vec<PluginDependency>>,

  /// Plugin specific config
  #[serde(default)]
  pub plugin_config: HashMap<String, Value>
}

impl PactPluginManifest {
  pub fn as_dependency(&self) -> PluginDependency {
    PluginDependency {
      name: self.name.clone(),
      version: Some(self.version.clone()),
      dependency_type: PluginDependencyType::Plugin
    }
  }
}

impl Default for PactPluginManifest {
  fn default() -> Self {
    PactPluginManifest {
      plugin_dir: "".to_string(),
      plugin_interface_version: 1,
      name: "".to_string(),
      version: "".to_string(),
      executable_type: "".to_string(),
      minimum_required_version: None,
      entry_point: "".to_string(),
      entry_points: Default::default(),
      args: None,
      dependencies: None,
      plugin_config: Default::default()
    }
  }
}

/// Request to compare the contents by a plugin
#[derive(Clone, Debug, Default)]
pub struct CompareContentRequest {
  /// The expected contents from the Pact interaction
  pub expected_contents: OptionalBody,
  /// The actual contents that was received
  pub actual_contents: OptionalBody,
  /// Where there are keys or attributes in the data, indicates whether unexpected values are allowed
  pub allow_unexpected_keys: bool,
  /// Matching rules that apply
  pub matching_rules: HashMap<DocPath, RuleList>,
  /// Plugin configuration form the Pact
  pub plugin_configuration: Option<PluginInteractionConfig>
}

/// Result of comparing the contents by a plugin
#[derive(Clone, Debug)]
pub enum CompareContentResult {
  /// An error occurred trying to compare the contents
  Error(String),
  /// The content type was incorrect
  TypeMismatch(String, String),
  /// There were mismatched results
  Mismatches(HashMap<String, Vec<ContentMismatch>>),
  /// All OK
  OK
}

/// Request to generate the contents by a plugin
#[derive(Clone, Debug, Default)]
pub struct GenerateContentRequest {
  /// The content type to generate
  pub content_type: ContentType,
  /// Example contents to replace
  pub content: OptionalBody,
  /// Generators that apply to the contents
  pub generators: HashMap<String, Generator>,
  /// Global plugin data stored in the Pact file
  pub plugin_data: Option<HashMap<String, Value>>,
  /// Plugin data stored on the interaction
  pub interaction_data: Option<HashMap<String, Value>>,
  /// Test context in effect
  pub test_context: HashMap<String, Value>
}

/// Pact Plugin trait
#[async_trait]
pub trait PactPlugin: Debug {
  /// Manifest for this plugin
  fn manifest(&self) -> PactPluginManifest;

  /// Shutdown this plugin. For plugins running as a separate process, this will kill the plugin process.
  fn kill(&self);

  /// Update the access count of the plugin. This is used for plugins running as a separate process,
  /// so that the child process can be cleaned up when no longer used.
  fn update_access(&mut self);

  /// Decrement and return the access count for the plugin
  fn drop_access(&mut self) -> usize;

  /// Clone the plugin and return it wrapped in a Box
  fn boxed(&self) -> Box<dyn PactPlugin + Send + Sync>;

  /// Clone the plugin and return it wrapped in an Arc
  fn arced(&self) -> Arc<dyn PactPlugin + Send + Sync>;

  /// Publish the current catalogue to the plugin
  async fn publish_updated_catalogue(&self, catalogue: &[CatalogueEntry]) -> anyhow::Result<()>;

  /// Send a generate content request to the plugin
  async fn generate_contents(&self, request: GenerateContentRequest) -> anyhow::Result<OptionalBody>;

  /// Send a match content request to the plugin
  async fn match_contents(&self, request: CompareContentRequest) -> anyhow::Result<CompareContentResult>;

  /// Get the plugin to configure the interaction contents for the interaction part based on the
  /// provided definition. Returns the interaction data used to create the Pact interaction.
  async fn configure_interaction(
    &self,
    content_type: &ContentType,
    definition: &HashMap<String, Value>
  ) -> anyhow::Result<(Vec<InteractionContents>, Option<PluginConfiguration>)>;

  /// Execute the verification for the interaction returning the verification results.
  async fn verify_interaction(
    &self,
    pact: &V4Pact,
    interaction: &(dyn V4Interaction + Send + Sync),
    verification_data: &InteractionVerificationData,
    config: &HashMap<String, Value>
  ) -> anyhow::Result<InteractionVerificationResult>;

  /// Sets up a transport request to be made. This is the first phase when verifying, and it allows the
  /// users to add additional values to any requests that are made.
  async fn prepare_interaction_for_verification(
    &self,
    pact: &V4Pact,
    interaction: &(dyn V4Interaction + Send + Sync),
    context: &HashMap<String, Value>
  ) -> anyhow::Result<InteractionVerificationData>;

  /// Starts a mock server given the Pact, mock server config and test context
  async fn start_mock_server(
    &self,
    config: &MockServerConfig,
    pact: Box<dyn Pact + Send + Sync>,
    test_context: HashMap<String, Value>
  ) -> anyhow::Result<MockServerDetails>;

  /// Gets the results from a running mock server.
  async fn get_mock_server_results(&self, mock_server_key: &str) -> anyhow::Result<Vec<MockServerResults>>;

  /// Shutdowns a running mock server. Will return any errors from the mock server.
  async fn shutdown_mock_server(&self, mock_server_key: &str) -> anyhow::Result<Vec<MockServerResults>>;
}

/// Plugin configuration to add to the matching context for an interaction
#[derive(Clone, Debug, PartialEq)]
pub struct PluginInteractionConfig {
  /// Global plugin config (Pact level)
  pub pact_configuration: HashMap<String, Value>,
  /// Interaction plugin config
  pub interaction_configuration: HashMap<String, Value>
}

#[cfg(test)]
pub(crate) mod tests {

}
