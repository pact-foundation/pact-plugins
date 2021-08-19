//! Manages the catalogue of features provided by plugins

use std::collections::{BTreeMap, HashMap};
use std::fmt::{self, Display, Formatter};
use std::sync::Mutex;

use itertools::Itertools;
use lazy_static::lazy_static;
use log::{debug, error};
use maplit::hashset;
use regex::Regex;
use serde::{Deserialize, Serialize};

use pact_models::content_types::ContentType;

use crate::content::ContentMatcher;
use crate::plugin_models::PactPluginManifest;
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
  /// Mock server
  MOCK_SERVER,
  /// Matching rule
  MATCHER,
  /// Generator
  INTERACTION
}

impl Display for CatalogueEntryType {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      CatalogueEntryType::CONTENT_MATCHER => write!(f, "content-matcher"),
      CatalogueEntryType::CONTENT_GENERATOR => write!(f, "content-generator"),
      CatalogueEntryType::MOCK_SERVER => write!(f, "mock-server"),
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
      "mock-server" => CatalogueEntryType::MOCK_SERVER,
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
  let mut guard = CATALOGUE_REGISTER.lock().unwrap();

  for entry in catalogue_list {
    let entry_type = CatalogueEntryType::from(entry.r#type.clone());
    let key = format!("plugin/{}/{}/{}", plugin.name, entry_type, entry.key);
    guard.insert(key.clone(), CatalogueEntry {
      entry_type,
      provider_type: CatalogueEntryProviderType::PLUGIN,
      plugin: Some(plugin.clone()),
      key: key.clone(),
      values: entry.values.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    });
  }

  debug!("Updated catalogue entries:\n{}", guard.keys().sorted().join("\n"))
}

/// Register the core Pact framework entries in the global catalogue
pub fn register_core_entries(entries: &Vec<CatalogueEntry>) {
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

/// Remove entries for a plugin
pub fn remove_plugin_entries(name: &String) {
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
  guard.values().find(|entry| {
    if entry.entry_type == CatalogueEntryType::CONTENT_MATCHER {
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
  let content_type = ContentType { attributes: BTreeMap::default(), .. content_type.clone() }.to_string();
  match Regex::new(pattern) {
    Ok(regex) => regex.is_match(content_type.as_str()),
    Err(err) => {
      error!("Failed to parse '{}' as a regex - {}", pattern, err);
      false
    }
  }
}

// TODO
///// Find a content genetrator in the global catalogue for the provided content type
// pub fn find_content_generator(content_type: ContentType) -> Option<ContentGenerator> {
//   val catalogueEntry = catalogue.values.find { entry ->
//     if (entry.type == CatalogueEntryType.CONTENT_GENERATOR) {
//       val contentTypes = entry.values["content-types"]?.split(';')
//       if (contentTypes.isNullOrEmpty()) {
//         false
//       } else {
//         contentTypes.any { contentType.matches(it) }
//       }
//     } else {
//       false
//     }
//   }
//   return if (catalogueEntry != null)
//     CatalogueContentGenerator(catalogueEntry)
//   else null
// }
