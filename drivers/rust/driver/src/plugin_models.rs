//! Models for representing plugins

use serde::{Deserialize, Serialize};

use crate::child_process::ChildPluginProcess;

/// Type of plugin dependencies
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
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
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
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
  /// Dependencies required to invoke the plugin
  pub dependencies: Option<Vec<PluginDependency>>
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

/// Running plugin details
#[derive(Debug, Clone)]
pub struct PactPlugin {
  /// Manifest for this plugin
  pub manifest: PactPluginManifest,
  /// Running child process
  pub child: ChildPluginProcess,
}

impl PactPlugin {
  /// Port the plugin is running on
  pub fn port(&self) -> u16 {
    self.child.port()
  }
}
