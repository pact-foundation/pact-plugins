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
use tracing::{debug, error, instrument, trace};

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

/// Where a resolved catalogue entry's capability should be dispatched. Shared by every transport
/// that lets a plugin call back into a capability by catalogue entry key - the gRPC `PluginHost`
/// service, and the Lua/WASM host functions - so there is exactly one place that decides "who
/// provides this entry". See proposal 007 ("One resolver, [multiple] call directions").
#[derive(Debug, Clone)]
pub enum ResolvedCapability {
  /// A host-registered core handler, keyed by the unprefixed catalogue entry key.
  Core(String),
  /// A running plugin, identified by its manifest.
  Plugin(Box<PactPluginManifest>)
}

/// Resolve a callback's catalogue entry key to a dispatch target, the same way
/// [`crate::content::ContentMatcher::is_core`]/[`crate::content::ContentGenerator::is_core`] do
/// for the driver's own outbound calls.
///
/// `entry_key` is matched by suffix against the full catalogue key (e.g. a plugin passing
/// `"xml"` matches `"core/content-matcher/xml"`), the same lookup [`lookup_entry`] does - except
/// unlike [`lookup_entry`], this does not just take the first `HashMap` hit if more than one
/// entry matches the suffix. A short, unqualified `entry_key` could otherwise coincidentally
/// match more than one entry of the *same* `expected_type` (e.g. a plugin registering its own
/// `content-matcher/xml` alongside a host-registered core `content-matcher/xml`), and `HashMap`
/// iteration order is randomised per process - silently picking one would make the dispatch
/// target non-deterministic across restarts. `expected_type` still guards against the *wrong*
/// capability shape (a content-generator registered under the same name as an unrelated
/// content-matcher), mirroring the explicit `entry_type` check [`find_content_matcher`]/
/// [`find_content_generator`] already do.
pub fn resolve_capability(entry_key: &str, expected_type: CatalogueEntryType) -> anyhow::Result<ResolvedCapability> {
  let candidates: Vec<(String, CatalogueEntry)> = {
    let inner = CATALOGUE_REGISTER.lock().unwrap();
    inner.iter()
      .filter(|(k, _)| k.ends_with(entry_key))
      .map(|(k, v)| (k.clone(), v.clone()))
      .collect()
  };

  let mut of_expected_type = candidates.iter().filter(|(_, entry)| entry.entry_type == expected_type);
  let entry = match (of_expected_type.next(), of_expected_type.next()) {
    (None, _) => return match candidates.first() {
      Some((_, entry)) => Err(anyhow::anyhow!(
        "Catalogue entry '{}' is a {:?}, not a {:?}", entry_key, entry.entry_type, expected_type
      )),
      None => Err(anyhow::anyhow!("No catalogue entry found for key '{}'", entry_key))
    },
    (Some(only), None) => &only.1,
    (Some(first), Some(second)) => {
      let mut keys: Vec<&str> = std::iter::once(first).chain(std::iter::once(second))
        .chain(of_expected_type)
        .map(|(k, _)| k.as_str())
        .collect();
      keys.sort_unstable();
      return Err(anyhow::anyhow!(
        "Ambiguous catalogue entry key '{}': matches multiple entries ({}) - register it under a more specific key",
        entry_key, keys.join(", ")
      ));
    }
  };

  match entry.provider_type {
    CatalogueEntryProviderType::CORE => Ok(ResolvedCapability::Core(entry.key.clone())),
    CatalogueEntryProviderType::PLUGIN => entry.plugin.clone()
      .map(|manifest| ResolvedCapability::Plugin(Box::new(manifest)))
      .ok_or_else(|| anyhow::anyhow!("Catalogue entry '{}' has no plugin manifest", entry_key))
  }
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
#[instrument(level = "trace", skip(content_type))]
pub fn find_content_matcher<CT: Into<String>>(content_type: CT) -> Option<ContentMatcher> {
  let content_type_str = content_type.into();
  debug!("Looking for a content matcher for {}", content_type_str);
  let content_type = match ContentType::parse(content_type_str.as_str()) {
    Ok(ct) => ct,
    Err(err) => {
      error!("'{}' is not a valid content type", err);
      return None;
    }
  };
  let guard = CATALOGUE_REGISTER.lock().unwrap();
  trace!("Catalogue has {} entries", guard.len());
  guard.values().find(|entry| {
    trace!("Catalogue entry {:?}", entry);
    if entry.entry_type == CatalogueEntryType::CONTENT_MATCHER {
      trace!("Catalogue entry is a content matcher for {:?}", entry.values.get("content-types"));
      if let Some(content_types) = entry.values.get("content-types") {
        content_types.split(";").any(|ct| matches_pattern(ct.trim(), &content_type))
      } else {
        false
      }
    } else {
      false
    }
  }).map(|entry| ContentMatcher { catalogue_entry: entry.clone() })
}

/// Checks if a registered content-type pattern matches a content type. The pattern is
/// matched as a regex against the base type (i.e. with any parameters like `charset`
/// stripped), anchored at both ends (the whole base type must match, not just a substring) -
/// this must stay consistent with the equivalent check in the JVM driver's
/// `CatalogueManager.matches`, so that a plugin's catalogue registration behaves the same way
/// regardless of which driver loaded it. Regex metacharacters in a content type (most
/// commonly `+`, as in a `+json`/`+xml` structured syntax suffix) need to be escaped by the
/// plugin author for a literal match.
fn matches_pattern(pattern: &str, content_type: &ContentType) -> bool {
  // Deliberately not `content_type.base_type()`: that replaces the subtype with the
  // structured syntax suffix (e.g. "application/jwt+json" -> "application/json"), which is
  // useful for deciding how to *parse* a body but wrong here - it would make two unrelated
  // "+json" content types register as the same catalogue entry. Just strip attributes
  // (e.g. `charset`), keeping the type/subtype+suffix as the plugin actually registered it.
  let base_type = match &content_type.suffix {
    Some(suffix) => format!("{}/{}+{}", content_type.main_type, content_type.sub_type, suffix),
    None => format!("{}/{}", content_type.main_type, content_type.sub_type)
  };
  match Regex::new(&format!("^(?:{})$", pattern)) {
    Ok(regex) => regex.is_match(base_type.as_str()),
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

/// Returns catalogue entries provided by the core host framework (excludes plugin entries)
pub fn core_entries() -> Vec<CatalogueEntry> {
  let guard = CATALOGUE_REGISTER.lock().unwrap();
  guard.values()
    .filter(|entry| entry.provider_type == CatalogueEntryProviderType::CORE)
    .cloned()
    .collect()
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

  #[test]
  fn find_content_matcher_requires_the_whole_base_type_to_match() {
    let manifest = PactPluginManifest {
      name: "find_content_matcher_requires_the_whole_base_type_to_match".to_string(),
      .. PactPluginManifest::default()
    };
    let entries = vec![
      ProtoCatalogueEntry {
        r#type: catalogue_entry::EntryType::ContentMatcher as i32,
        key: "jwt".to_string(),
        // "+" must be escaped, otherwise it's a regex quantifier, not a literal character
        values: hashmap!{ "content-types".to_string() => "application/jwt;application/jwt\\+json".to_string() }
      }
    ];
    register_plugin_entries(&manifest, &entries);

    let exact_match = find_content_matcher("application/jwt+json");
    let with_params = find_content_matcher("application/jwt+json;charset=utf-8");
    let longer_type = find_content_matcher("application/jwt+jsonextra");
    let unrelated_type = find_content_matcher("application/json");

    remove_plugin_entries("find_content_matcher_requires_the_whole_base_type_to_match");

    expect!(exact_match).to(be_some());
    expect!(with_params).to(be_some());
    expect!(longer_type).to(be_none());
    expect!(unrelated_type).to(be_none());
  }

  #[test]
  fn resolve_capability_resolves_an_unambiguous_core_entry() {
    let key = "resolve_capability_resolves_an_unambiguous_core_entry";
    register_core_entries(&vec![CatalogueEntry {
      entry_type: CatalogueEntryType::CONTENT_MATCHER,
      provider_type: CatalogueEntryProviderType::CORE,
      plugin: None,
      key: key.to_string(),
      values: hashmap!{}
    }]);

    let resolved = resolve_capability(key, CatalogueEntryType::CONTENT_MATCHER).unwrap();

    let core_key = match resolved {
      ResolvedCapability::Core(core_key) => core_key,
      ResolvedCapability::Plugin(_) => panic!("expected a Core resolution, got Plugin")
    };
    expect!(core_key).to(be_equal_to(key.to_string()));
  }

  #[test]
  fn resolve_capability_returns_a_clear_error_for_an_unregistered_key() {
    let result = resolve_capability(
      "resolve_capability_returns_a_clear_error_for_an_unregistered_key",
      CatalogueEntryType::CONTENT_MATCHER
    );

    let err = result.expect_err("expected an error for an unregistered key");
    expect!(err.to_string().contains("No catalogue entry found")).to(be_true());
  }

  #[test]
  fn resolve_capability_returns_a_clear_error_for_the_wrong_capability_shape() {
    let key = "resolve_capability_returns_a_clear_error_for_the_wrong_capability_shape";
    register_core_entries(&vec![CatalogueEntry {
      entry_type: CatalogueEntryType::CONTENT_GENERATOR,
      provider_type: CatalogueEntryProviderType::CORE,
      plugin: None,
      key: key.to_string(),
      values: hashmap!{}
    }]);

    let result = resolve_capability(key, CatalogueEntryType::CONTENT_MATCHER);

    let err = result.expect_err("expected an error when the entry is a generator, not a matcher");
    expect!(err.to_string().contains("is a CONTENT_GENERATOR, not a CONTENT_MATCHER")).to(be_true());
  }

  #[test]
  fn resolve_capability_rejects_an_ambiguous_key_shared_by_a_core_and_a_plugin_entry() {
    let key = "resolve_capability_rejects_an_ambiguous_key_shared_by_a_core_and_a_plugin_entry";
    let manifest = PactPluginManifest {
      name: "resolve_capability_rejects_an_ambiguous_key_shared_by_a_core_and_a_plugin_entry".to_string(),
      .. PactPluginManifest::default()
    };
    register_core_entries(&vec![CatalogueEntry {
      entry_type: CatalogueEntryType::CONTENT_MATCHER,
      provider_type: CatalogueEntryProviderType::CORE,
      plugin: None,
      key: key.to_string(),
      values: hashmap!{}
    }]);
    register_plugin_entries(&manifest, &vec![ProtoCatalogueEntry {
      r#type: catalogue_entry::EntryType::ContentMatcher as i32,
      key: key.to_string(),
      values: hashmap!{}
    }]);

    let result = resolve_capability(key, CatalogueEntryType::CONTENT_MATCHER);

    remove_plugin_entries(&manifest.name);

    let err = result.expect_err("expected an error for a key matching more than one entry");
    expect!(err.to_string().contains("Ambiguous catalogue entry key")).to(be_true());
  }
}
