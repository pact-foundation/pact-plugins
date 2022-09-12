//! Manages the catalogue of features provided by plugins

use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::sync::Mutex;

use itertools::Itertools;
use lazy_static::lazy_static;
use maplit::hashset;
use pact_models::content_types::ContentType;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, trace};

use crate::content::{ContentGenerator, ContentMatcher};
use crate::plugin_models::PactPluginManifest;
use crate::proto::catalogue_entry::EntryType;
use crate::proto::CatalogueEntry as ProtoCatalogueEntry;

lazy_static! {
  static ref CATALOGUE_REGISTER: Mutex<HashMap<String, CatalogueEntry>> = Mutex::new(HashMap::new());
}

/// Type of catalogue entry
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum CatalogueEntryType {
  /// Content matcher (based on content type)
  CONTENT_MATCHER,
  /// Content generator (based on content type)
  CONTENT_GENERATOR,
  /// Network transport
  TRANSPORT,
  /// Matching rule
  MATCHER,
  /// Generator
  INTERACTION
}

impl CatalogueEntryType {
  /// Return the protobuf type for this entry type
  pub fn to_proto_type(&self) -> EntryType {
    match self {
      CatalogueEntryType::CONTENT_MATCHER => EntryType::ContentMatcher,
      CatalogueEntryType::CONTENT_GENERATOR => EntryType::ContentGenerator,
      CatalogueEntryType::TRANSPORT => EntryType::Transport,
      CatalogueEntryType::MATCHER => EntryType::Matcher,
      CatalogueEntryType::INTERACTION => EntryType::Interaction
    }
  }
}

impl Display for CatalogueEntryType {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      CatalogueEntryType::CONTENT_MATCHER => write!(f, "content-matcher"),
      CatalogueEntryType::CONTENT_GENERATOR => write!(f, "content-generator"),
      CatalogueEntryType::TRANSPORT => write!(f, "transport"),
      CatalogueEntryType::MATCHER => write!(f, "matcher"),
      CatalogueEntryType::INTERACTION => write!(f, "interaction"),
    }
  }
}

impl From<&str> for CatalogueEntryType {
  fn from(s: &str) -> Self {
    match s {
      "content-matcher" => CatalogueEntryType::CONTENT_MATCHER,
      "content-generator" => CatalogueEntryType::CONTENT_GENERATOR,
      "interaction" => CatalogueEntryType::INTERACTION,
      "matcher" => CatalogueEntryType::MATCHER,
      "transport" => CatalogueEntryType::TRANSPORT,
      _ => {
        let message = format!("'{}' is not a valid CatalogueEntryType value", s);
        error!("{}", message);
        panic!("{}", message)
      }
    }
  }
}

impl From<String> for CatalogueEntryType {
  fn from(s: String) -> Self {
    Self::from(s.as_str())
  }
}

impl From<EntryType> for CatalogueEntryType {
  fn from(t: EntryType) -> Self {
    match t {
      EntryType::ContentMatcher => CatalogueEntryType::CONTENT_MATCHER,
      EntryType::ContentGenerator => CatalogueEntryType::CONTENT_GENERATOR,
      EntryType::Transport => CatalogueEntryType::TRANSPORT,
      EntryType::Matcher => CatalogueEntryType::MATCHER,
      EntryType::Interaction => CatalogueEntryType::INTERACTION
    }
  }
}

/// Provider of the catalogue entry
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum CatalogueEntryProviderType {
  /// Core Pact framework
  CORE,
  /// Plugin
  PLUGIN
}

/// Catalogue entry
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueEntry {
  /// Type of entry
  pub entry_type: CatalogueEntryType,
  /// Provider of the entry
  pub provider_type: CatalogueEntryProviderType,
  /// Plugin manifest
  pub plugin: Option<PactPluginManifest>,
  /// Entry key
  pub key: String,
  /// assocaited Entry values
  pub values: HashMap<String, String>
}

/// Register the entries in the global catalogue
pub fn register_plugin_entries(plugin: &PactPluginManifest, catalogue_list: &Vec<ProtoCatalogueEntry>) {
  trace!("register_plugin_entries({:?}, {:?})", plugin, catalogue_list);

  let mut guard = CATALOGUE_REGISTER.lock().unwrap();

  for entry in catalogue_list {
    let entry_type = CatalogueEntryType::from(entry.r#type());
    let key = format!("plugin/{}/{}/{}", plugin.name, entry_type, entry.key);
    guard.insert(key.clone(), CatalogueEntry {
      entry_type,
      provider_type: CatalogueEntryProviderType::PLUGIN,
      plugin: Some(plugin.clone()),
      key: entry.key.clone(),
      values: entry.values.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    });
  }

  debug!("Updated catalogue entries:\n{}", guard.keys().sorted().join("\n"))
}

/// Register the core Pact framework entries in the global catalogue
pub fn register_core_entries(entries: &Vec<CatalogueEntry>) {
  trace!("register_core_entries({:?})", entries);

  let mut inner = CATALOGUE_REGISTER.lock().unwrap();

  let mut updated_keys = hashset!();
  for entry in entries {
    let key = format!("core/{}/{}", entry.entry_type, entry.key);
    if !inner.contains_key(&key) {
      inner.insert(key.clone(), entry.clone());
      updated_keys.insert(key.clone());
    }
  }

  if !updated_keys.is_empty() {
    debug!("Updated catalogue entries:\n{}", updated_keys.iter().sorted().join("\n"));
  }
}

/// Lookup an entry in the catalogue by the key. Will find the first entry that ends with the
/// given key.
pub fn lookup_entry(key: &str) -> Option<CatalogueEntry> {
  let inner = CATALOGUE_REGISTER.lock().unwrap();
  inner.iter()
    .find(|(k, _)| k.ends_with(key))
    .map(|(_, v)| v.clone())
}

/// Remove all entries for a plugin given the plugin name
pub fn remove_plugin_entries(name: &str) {
  trace!("remove_plugin_entries({})", name);

  let prefix = format!("plugin/{}/", name);
  let keys: Vec<String> = {
    let guard = CATALOGUE_REGISTER.lock().unwrap();
    guard.keys()
      .filter(|key| key.starts_with(&prefix))
      .cloned()
      .collect()
  };

  let mut guard = CATALOGUE_REGISTER.lock().unwrap();
  for key in keys {
    guard.remove(&key);
  }

  debug!("Removed all catalogue entries for plugin {}", name);
}

/// Find a content matcher in the global catalogue for the provided content type
pub fn find_content_matcher(content_type: &ContentType) -> Option<ContentMatcher> {
  debug!("Looking for a content matcher for {}", content_type);
  let guard = CATALOGUE_REGISTER.lock().unwrap();
  trace!("Catalogue has {} entries", guard.len());
  guard.values().find(|entry| {
    trace!("Catalogue entry {:?}", entry);
    if entry.entry_type == CatalogueEntryType::CONTENT_MATCHER {
      trace!("Catalogue entry is a content matcher for {:?}", entry.values.get("content-types"));
      if let Some(content_types) = entry.values.get("content-types") {
        content_types.split(";").any(|ct| matches_pattern(ct.trim(), content_type))
      } else {
        false
      }
    } else {
      false
    }
  }).map(|entry| ContentMatcher { catalogue_entry: entry.clone() })
}

fn matches_pattern(pattern: &str, content_type: &ContentType) -> bool {
  let base_type = content_type.base_type().to_string();
  match Regex::new(pattern) {
    Ok(regex) => regex.is_match(content_type.to_string().as_str()) || regex.is_match(base_type.as_str()),
    Err(err) => {
      error!("Failed to parse '{}' as a regex - {}", pattern, err);
      false
    }
  }
}

/// Find a content generator in the global catalogue for the provided content type
pub fn find_content_generator(content_type: &ContentType) -> Option<ContentGenerator> {
  debug!("Looking for a content generator for {}", content_type);
  let guard = CATALOGUE_REGISTER.lock().unwrap();
  guard.values().find(|entry| {
    if entry.entry_type == CatalogueEntryType::CONTENT_GENERATOR {
      if let Some(content_types) = entry.values.get("content-types") {
        content_types.split(";").any(|ct| matches_pattern(ct.trim(), content_type))
      } else {
        false
      }
    } else {
      false
    }
  }).map(|entry| ContentGenerator { catalogue_entry: entry.clone() })
}

/// Returns a copy of all catalogue entries
pub fn all_entries() -> Vec<CatalogueEntry> {
  let guard = CATALOGUE_REGISTER.lock().unwrap();
  guard.values().cloned().collect()
}

#[cfg(test)]
mod tests {
  use expectest::prelude::*;
  use maplit::hashmap;

  use crate::proto::catalogue_entry;

  use super::*;

  #[test]
  fn sets_plugin_catalogue_entries_correctly() {
    // Given
    let manifest = PactPluginManifest {
      name: "sets_plugin_catalogue_entries_correctly".to_string(),
      .. PactPluginManifest::default()
    };
    let entries = vec![
      ProtoCatalogueEntry {
        r#type: catalogue_entry::EntryType::ContentMatcher as i32,
        key: "protobuf".to_string(),
        values: hashmap!{ "content-types".to_string() => "application/protobuf;application/grpc".to_string() }
      },
      ProtoCatalogueEntry {
        r#type: catalogue_entry::EntryType::ContentGenerator as i32,
        key: "protobuf".to_string(),
        values: hashmap!{ "content-types".to_string() => "application/protobuf;application/grpc".to_string() }
      },
      ProtoCatalogueEntry {
        r#type: catalogue_entry::EntryType::Transport as i32,
        key: "grpc".to_string(),
        values: hashmap!{}
      }
    ];

    // When
    register_plugin_entries(&manifest, &entries);

    // Then
    let matcher_entry = lookup_entry("content-matcher/protobuf");
    let generator_entry = lookup_entry("content-generator/protobuf");
    let transport_entry = lookup_entry("transport/grpc");

    remove_plugin_entries("sets_plugin_catalogue_entries_correctly");

    expect!(matcher_entry).to(be_some().value(CatalogueEntry {
      entry_type: CatalogueEntryType::CONTENT_MATCHER,
      provider_type: CatalogueEntryProviderType::PLUGIN,
      plugin: Some(manifest.clone()),
      key: "protobuf".to_string(),
      values: hashmap!{ "content-types".to_string() => "application/protobuf;application/grpc".to_string() }
    }));
    expect!(generator_entry).to(be_some().value(CatalogueEntry {
      entry_type: CatalogueEntryType::CONTENT_GENERATOR,
      provider_type: CatalogueEntryProviderType::PLUGIN,
      plugin: Some(manifest.clone()),
      key: "protobuf".to_string(),
      values: hashmap!{ "content-types".to_string() => "application/protobuf;application/grpc".to_string() }
    }));
    expect!(transport_entry).to(be_some().value(CatalogueEntry {
      entry_type: CatalogueEntryType::TRANSPORT,
      provider_type: CatalogueEntryProviderType::PLUGIN,
      plugin: Some(manifest.clone()),
      key: "grpc".to_string(),
      values: hashmap!{}
    }));
  }
}
