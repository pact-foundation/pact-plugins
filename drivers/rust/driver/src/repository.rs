//! Module for dealing with the plugin repository

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::plugin_models::PactPluginManifest;

pub const DEFAULT_INDEX: &str = include_str!("../repository.index");

/// Struct representing the plugin repository index file
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginRepositoryIndex {
  /// Version of this index file
  pub index_version: usize,

  /// File format version of the index file
  pub format_version: usize,

  /// Timestamp (in UTC) that the file was created/updated
  pub timestamp: DateTime<Utc>,

  /// Plugin entries
  pub entries: HashMap<String, PluginEntry>
}

/// Struct to store the plugin version entries
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginEntry {
  /// Name of the plugin
  pub name: String,
  /// Latest version
  pub latest_version: String,
  /// All the plugin versions
  pub versions: Vec<PluginVersion>
}

/// Struct to store the plugin versions
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginVersion {
  /// Version of the plugin
  pub version: String,
  /// Source the manifest was loaded from
  pub source: ManifestSource,
  /// Manifest
  pub manifest: Option<PactPluginManifest>
}

/// Source that the plugin is loaded from
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum ManifestSource {
  /// Loaded from a file
  File(String),

  /// Loaded from a GitHub release
  GitHubRelease(String)
}

impl ManifestSource {
  /// Returns the name of the plugin source
  pub fn name(&self) -> String {
    match self {
      ManifestSource::File(_) => "file".to_string(),
      ManifestSource::GitHubRelease(_) => "GitHub release".to_string()
    }
  }

  /// Returns the associated value for the plugin source. For example, for a file source, returns
  /// the file path.
  pub fn value(&self) -> String {
    match self {
      ManifestSource::File(v) => v.clone(),
      ManifestSource::GitHubRelease(v) => v.clone()
    }
  }
}

impl PluginEntry {
  /// Create a new plugin entry from the provided manifest and source
  pub fn new(manifest: &PactPluginManifest, source: &ManifestSource) -> PluginEntry {
    PluginEntry {
      name: manifest.name.clone(),
      latest_version: manifest.version.clone(),
      versions: vec![PluginVersion {
        version: manifest.version.clone(),
        source: source.clone(),
        manifest: Some(manifest.clone())
      }]
    }
  }

  /// Adds the data from the plugin manifest as a version to the index
  pub fn add_version(&mut self, manifest: &PactPluginManifest, source: &ManifestSource) {
    if let Some(version) = self.versions.iter_mut()
      .find(|m| m.version == manifest.version) {
      version.source = source.clone();
      version.manifest = Some(manifest.clone());
    } else {
      self.versions.push(PluginVersion {
        version: manifest.version.clone(),
        source: source.clone(),
        manifest: Some(manifest.clone())
      });
    }
    self.update_latest_version();
  }

  fn update_latest_version(&mut self) {
    let latest_version = self.versions.iter()
      .max_by(|m1, m2| {
        let a = Version::parse(&m1.version).unwrap_or_else(|_| Version::new(0, 0, 0));
        let b = Version::parse(&m2.version).unwrap_or_else(|_| Version::new(0, 0, 0));
        a.cmp(&b)
      })
      .map(|m| m.version.clone())
      .unwrap_or_default();
    self.latest_version = latest_version.clone();
  }
}

impl Default for PluginRepositoryIndex {
  fn default() -> Self {
    PluginRepositoryIndex {
      index_version: 0,
      format_version: 0,
      timestamp: Utc::now(),
      entries: Default::default()
    }
  }
}
