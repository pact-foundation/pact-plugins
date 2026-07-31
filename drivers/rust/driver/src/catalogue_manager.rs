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
use tracing::{debug, error, instrument, trace, warn};

use crate::content::{ContentGenerator, ContentMatcher};
use crate::plugin_models::PactPluginManifest;
use crate::proto::catalogue_entry::EntryType;
use crate::proto::CatalogueEntry as ProtoCatalogueEntry;
use crate::proto_v2::catalogue_entry::EntryType as EntryTypeV2;

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
  /// Matching rule for a content field/value
  MATCHER,
  /// Type of interaction
  INTERACTION,
  /// Generator for a content field/value. Only representable on the V2 plugin interface - the V1
  /// `EntryType` enum has no equivalent. See proposal 006 (Field-level matchers and generators).
  GENERATOR
}

impl CatalogueEntryType {
  /// Return the protobuf type for this entry type.
  ///
  /// This maps to the *V1* enum, which cannot represent every entry type: `GENERATOR` has no V1
  /// equivalent and is reported as `Matcher`. Use [`CatalogueEntryType::to_proto_value`] instead,
  /// which returns the wire value and is lossless for both interface versions.
  #[deprecated(
    since = "1.2.0",
    note = "Use to_proto_value, which can represent entry types that only exist on the V2 interface"
  )]
  pub fn to_proto_type(&self) -> EntryType {
    match self {
      CatalogueEntryType::CONTENT_MATCHER => EntryType::ContentMatcher,
      CatalogueEntryType::CONTENT_GENERATOR => EntryType::ContentGenerator,
      CatalogueEntryType::TRANSPORT => EntryType::Transport,
      CatalogueEntryType::MATCHER => EntryType::Matcher,
      CatalogueEntryType::INTERACTION => EntryType::Interaction,
      CatalogueEntryType::GENERATOR => EntryType::Matcher
    }
  }

  /// The V2 protobuf enum for this entry type. V2 is the canonical source of the wire values: it
  /// mirrors V1 for the entry types both versions share, and adds the ones V1 never had.
  fn to_proto_enum(self) -> EntryTypeV2 {
    match self {
      CatalogueEntryType::CONTENT_MATCHER => EntryTypeV2::ContentMatcher,
      CatalogueEntryType::CONTENT_GENERATOR => EntryTypeV2::ContentGenerator,
      CatalogueEntryType::TRANSPORT => EntryTypeV2::Transport,
      CatalogueEntryType::MATCHER => EntryTypeV2::Matcher,
      CatalogueEntryType::INTERACTION => EntryTypeV2::Interaction,
      CatalogueEntryType::GENERATOR => EntryTypeV2::Generator
    }
  }

  fn from_proto_enum(entry_type: EntryTypeV2) -> CatalogueEntryType {
    match entry_type {
      EntryTypeV2::ContentMatcher => CatalogueEntryType::CONTENT_MATCHER,
      EntryTypeV2::ContentGenerator => CatalogueEntryType::CONTENT_GENERATOR,
      EntryTypeV2::Transport => CatalogueEntryType::TRANSPORT,
      EntryTypeV2::Matcher => CatalogueEntryType::MATCHER,
      EntryTypeV2::Interaction => CatalogueEntryType::INTERACTION,
      EntryTypeV2::Generator => CatalogueEntryType::GENERATOR
    }
  }

  /// The protobuf enum value for this entry type, for setting the `type` field of a
  /// `CatalogueEntry` message. Lossless for every entry type, including those a V1 plugin will
  /// not recognise - a V1 plugin decodes an unknown value as an unrecognised enum and ignores the
  /// entry, which is the correct outcome, whereas mapping it onto some other V1 type would have
  /// the plugin act on an entry that is not what it thinks it is.
  pub fn to_proto_value(self) -> i32 {
    self.to_proto_enum() as i32
  }

  /// The entry type for a protobuf enum value, or `None` if the value is not one this driver
  /// understands (an entry type added by a later interface version). Deliberately not defaulting
  /// to `CONTENT_MATCHER` the way prost's generated accessor does: silently mis-typing an entry
  /// is worse than ignoring one.
  pub fn from_proto_value(value: i32) -> Option<CatalogueEntryType> {
    EntryTypeV2::try_from(value).ok().map(CatalogueEntryType::from_proto_enum)
  }

  /// The protobuf enum value name for this entry type, e.g. `"CONTENT_MATCHER"`. This is the form
  /// a Lua plugin uses in the catalogue entries returned from its `init` function.
  pub fn as_proto_name(&self) -> &'static str {
    self.to_proto_enum().as_str_name()
  }

  /// The entry type for a protobuf enum value name, e.g. `"CONTENT_MATCHER"`, or `None` if the
  /// name is not one this driver understands.
  pub fn from_proto_name(name: &str) -> Option<CatalogueEntryType> {
    EntryTypeV2::from_str_name(name).map(CatalogueEntryType::from_proto_enum)
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
      CatalogueEntryType::GENERATOR => write!(f, "generator"),
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
      "generator" => CatalogueEntryType::GENERATOR,
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
    // Deliberately reading the raw field rather than prost's `entry.r#type()` accessor: the
    // accessor is generated against the V1 enum and maps anything it doesn't recognise to the
    // default (`CONTENT_MATCHER`), which would silently register a V2-only entry type - a
    // `GENERATOR`, say - as a content matcher.
    let entry_type = match CatalogueEntryType::from_proto_value(entry.r#type) {
      Some(entry_type) => entry_type,
      None => {
        warn!(
          "Ignoring catalogue entry '{}' from plugin '{}': {} is not a catalogue entry type this driver understands",
          entry.key, plugin.name, entry.r#type
        );
        continue;
      }
    };
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

/// Lookup an entry in the catalogue by the key, matched the same way [`resolve_capability`] does:
/// by name first - the whole catalogue key, or a trailing run of its `/`-separated components -
/// then against the core catalogue's versioned naming convention.
///
/// Unlike [`resolve_capability`], this takes the first match when more than one entry matches, and
/// `HashMap` iteration order is randomised per process. Prefer [`resolve_capability`] wherever the
/// expected entry type is known and a deterministic answer matters.
pub fn lookup_entry(key: &str) -> Option<CatalogueEntry> {
  let inner = CATALOGUE_REGISTER.lock().unwrap();
  inner.iter()
    .find(|(k, _)| names_catalogue_key(k, key))
    .or_else(|| inner.iter().find(|(_, entry)| names_versioned_core_key(entry, key)))
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

/// Does `entry_key` name this catalogue key? Either the whole key (`core/content-matcher/xml`),
/// or a trailing run of its `/`-separated components (`content-matcher/xml`, or just `xml`).
///
/// Components are compared whole, never as substrings: `"type"` does not name
/// `core/matcher/v2-type`. That distinction is the point - the core catalogue prefixes the Pact
/// specification version a matcher was introduced in to its name, so a substring match would let
/// an unqualified name collide with a versioned core key it has nothing to do with (a plugin's
/// own `matcher/date` against `core/matcher/v3-date`, say). The versioned form is handled
/// deliberately by [`names_versioned_core_key`] instead, as a fallback rather than a coincidence.
fn names_catalogue_key(catalogue_key: &str, entry_key: &str) -> bool {
  let key_parts = catalogue_key.split('/').collect::<Vec<_>>();
  let query_parts = entry_key.split('/').collect::<Vec<_>>();
  query_parts.len() <= key_parts.len()
    && key_parts[key_parts.len() - query_parts.len()..] == query_parts[..]
}

/// Does `entry_key` name this entry under the core catalogue's versioned naming convention - the
/// Pact specification version the rule was introduced in, prefixed to its name (`v2-type`,
/// `v3-date`, `v4-not-empty`)? This is what lets a caller ask for `type` and get `v2-type`
/// without having to know which specification version introduced it.
///
/// Only the whole name after the version prefix counts, so `type` names `v2-type` but not
/// `v3-content-type` or `v2-min-type` - those are the `content-type` and `min-type` rules.
///
/// The convention only applies to matching rules and generators. Content matchers, content
/// generators and transports are registered under plain names (`xml`, `json`, `grpc`), so a
/// leading `v<n>-` there is part of the name rather than a version, and stripping it would be
/// wrong.
fn names_versioned_core_key(entry: &CatalogueEntry, entry_key: &str) -> bool {
  if entry.entry_type != CatalogueEntryType::MATCHER && entry.entry_type != CatalogueEntryType::GENERATOR {
    return false;
  }
  match entry.key.split_once('-') {
    Some((version, name)) => name == entry_key
      && version.len() > 1
      && version.starts_with('v')
      && version[1..].chars().all(|c| c.is_ascii_digit()),
    None => false
  }
}

/// Resolve a callback's catalogue entry key to a dispatch target, the same way
/// [`crate::content::ContentMatcher::is_core`]/[`crate::content::ContentGenerator::is_core`] do
/// for the driver's own outbound calls.
///
/// `entry_key` is matched against the catalogue in two passes:
///
/// 1. by name - the whole catalogue key (`core/content-matcher/xml`) or a trailing run of its
///    components (`content-matcher/xml`, `xml`), compared component by component;
/// 2. failing that, against the core catalogue's versioned naming convention, so `type` resolves
///    to `v2-type` and `date` to `v3-date`.
///
/// Naming an entry directly always wins over the versioned fallback: if a plugin registers its
/// own `matcher/date`, `date` resolves to that plugin, and a caller that specifically wants the
/// core rule asks for `v3-date`.
///
/// Unlike [`lookup_entry`], this does not just take the first `HashMap` hit when more than one
/// entry matches. A short, unqualified `entry_key` can still match more than one entry of the
/// *same* `expected_type` (a plugin registering its own `content-matcher/xml` alongside a
/// host-registered core `content-matcher/xml`), and `HashMap` iteration order is randomised per
/// process - silently picking one would make the dispatch target non-deterministic across
/// restarts. `expected_type` still guards against the *wrong* capability shape (a
/// content-generator registered under the same name as an unrelated content-matcher), mirroring
/// the explicit `entry_type` check [`find_content_matcher`]/[`find_content_generator`] already do.
pub fn resolve_capability(entry_key: &str, expected_type: CatalogueEntryType) -> anyhow::Result<ResolvedCapability> {
  let entry = resolve_capability_entry(entry_key, expected_type)?;
  match entry.provider_type {
    CatalogueEntryProviderType::CORE => Ok(ResolvedCapability::Core(entry.key.clone())),
    CatalogueEntryProviderType::PLUGIN => entry.plugin.clone()
      .map(|manifest| ResolvedCapability::Plugin(Box::new(manifest)))
      .ok_or_else(|| anyhow::anyhow!("Catalogue entry '{}' has no plugin manifest", entry_key))
  }
}

/// Resolve a catalogue entry key to the entry itself, using the same two-pass matching
/// [`resolve_capability`] documents. Callers that need the entry rather than a dispatch target -
/// [`crate::field::find_field_matcher`] and [`crate::field::find_field_generator`], which wrap it
/// in a matcher/generator - use this directly.
pub fn resolve_capability_entry(entry_key: &str, expected_type: CatalogueEntryType) -> anyhow::Result<CatalogueEntry> {
  let all_entries: Vec<(String, CatalogueEntry)> = {
    let inner = CATALOGUE_REGISTER.lock().unwrap();
    inner.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
  };

  let named: Vec<(String, CatalogueEntry)> = all_entries.iter()
    .filter(|(k, _)| names_catalogue_key(k, entry_key))
    .cloned()
    .collect();
  let candidates = if named.iter().any(|(_, entry)| entry.entry_type == expected_type) {
    named
  } else {
    let versioned: Vec<(String, CatalogueEntry)> = all_entries.iter()
      .filter(|(_, entry)| names_versioned_core_key(entry, entry_key))
      .cloned()
      .collect();
    // Keep any wrong-typed direct matches for the diagnostic below if the fallback finds nothing
    if versioned.is_empty() { named } else { versioned }
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

  Ok(entry.clone())
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
  fn entry_type_proto_values_and_names_round_trip() {
    for entry_type in [
      CatalogueEntryType::CONTENT_MATCHER,
      CatalogueEntryType::CONTENT_GENERATOR,
      CatalogueEntryType::TRANSPORT,
      CatalogueEntryType::MATCHER,
      CatalogueEntryType::INTERACTION,
      CatalogueEntryType::GENERATOR
    ] {
      expect!(CatalogueEntryType::from_proto_value(entry_type.to_proto_value()))
        .to(be_some().value(entry_type));
      expect!(CatalogueEntryType::from_proto_name(entry_type.as_proto_name()))
        .to(be_some().value(entry_type));
      // The Display form round-trips too - it is what catalogue keys are built from
      expect!(CatalogueEntryType::from(entry_type.to_string().as_str())).to(be_equal_to(entry_type));
    }

    expect!(CatalogueEntryType::from_proto_value(99)).to(be_none());
    expect!(CatalogueEntryType::from_proto_name("NOT_AN_ENTRY_TYPE")).to(be_none());
  }

  #[test]
  fn entry_types_shared_with_v1_keep_their_v1_wire_values() {
    // The wire values come from the V2 enum, but the driver publishes one catalogue to every
    // running plugin, V1 ones included - so the values V1 knows about must not shift.
    expect!(CatalogueEntryType::CONTENT_MATCHER.to_proto_value())
      .to(be_equal_to(EntryType::ContentMatcher as i32));
    expect!(CatalogueEntryType::CONTENT_GENERATOR.to_proto_value())
      .to(be_equal_to(EntryType::ContentGenerator as i32));
    expect!(CatalogueEntryType::TRANSPORT.to_proto_value())
      .to(be_equal_to(EntryType::Transport as i32));
    expect!(CatalogueEntryType::MATCHER.to_proto_value())
      .to(be_equal_to(EntryType::Matcher as i32));
    expect!(CatalogueEntryType::INTERACTION.to_proto_value())
      .to(be_equal_to(EntryType::Interaction as i32));
  }

  #[test]
  fn registers_a_generator_entry_under_its_own_entry_type() {
    let name = "registers_a_generator_entry_under_its_own_entry_type";
    let manifest = PactPluginManifest { name: name.to_string(), .. PactPluginManifest::default() };
    // GENERATOR only exists on the V2 enum. This is exactly the case where prost's generated
    // `entry.r#type()` accessor - built against V1 - would report CONTENT_MATCHER instead.
    let entries = vec![
      ProtoCatalogueEntry {
        r#type: CatalogueEntryType::GENERATOR.to_proto_value(),
        key: name.to_string(),
        values: hashmap!{}
      }
    ];

    register_plugin_entries(&manifest, &entries);

    let entry = lookup_entry(&format!("generator/{}", name));
    let as_a_content_matcher = lookup_entry(&format!("content-matcher/{}", name));
    remove_plugin_entries(name);

    expect!(entry.map(|entry| entry.entry_type)).to(be_some().value(CatalogueEntryType::GENERATOR));
    expect!(as_a_content_matcher).to(be_none());
  }

  #[test]
  fn ignores_a_catalogue_entry_whose_type_this_driver_does_not_understand() {
    let name = "ignores_a_catalogue_entry_whose_type_this_driver_does_not_understand";
    let manifest = PactPluginManifest { name: name.to_string(), .. PactPluginManifest::default() };
    let entries = vec![
      ProtoCatalogueEntry { r#type: 99, key: name.to_string(), values: hashmap!{} }
    ];

    register_plugin_entries(&manifest, &entries);

    let registered = all_entries().into_iter().find(|entry| entry.key == name);
    remove_plugin_entries(name);

    expect!(registered).to(be_none());
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

  /// The core matcher entries as the Pact frameworks actually register them - the Pact
  /// specification version the rule was introduced in, prefixed to the name. Kept in sync with
  /// `MATCHER_CATALOGUE_ENTRIES` in pact_matching and `MatcherExecutor.kt` in Pact-JVM.
  fn register_core_matcher_entries() {
    let entries = ["v2-regex", "v2-type", "v3-number-type", "v3-integer-type", "v3-decimal-type",
      "v3-date", "v3-time", "v3-datetime", "v2-min-type", "v2-max-type", "v2-minmax-type",
      "v3-includes", "v3-null", "v4-equals-ignore-order", "v4-min-equals-ignore-order",
      "v4-max-equals-ignore-order", "v4-minmax-equals-ignore-order", "v3-content-type",
      "v4-array-contains", "v1-equality", "v4-not-empty", "v4-semver"]
      .iter()
      .map(|key| CatalogueEntry {
        entry_type: CatalogueEntryType::MATCHER,
        provider_type: CatalogueEntryProviderType::CORE,
        plugin: None,
        key: key.to_string(),
        values: hashmap!{}
      })
      .collect();
    register_core_entries(&entries);
  }

  fn resolved_core_key(entry_key: &str) -> String {
    match resolve_capability(entry_key, CatalogueEntryType::MATCHER) {
      Ok(ResolvedCapability::Core(key)) => key,
      other => panic!("expected '{}' to resolve to a core entry, got {:?}", entry_key, other)
    }
  }

  #[test]
  fn resolve_capability_falls_back_to_the_versioned_core_key() {
    register_core_matcher_entries();

    // The name a Pact file (and a plugin calling back) uses, without needing to know which
    // specification version introduced the rule
    expect!(resolved_core_key("type")).to(be_equal_to("v2-type".to_string()));
    expect!(resolved_core_key("regex")).to(be_equal_to("v2-regex".to_string()));
    expect!(resolved_core_key("date")).to(be_equal_to("v3-date".to_string()));
    expect!(resolved_core_key("equality")).to(be_equal_to("v1-equality".to_string()));
    expect!(resolved_core_key("semver")).to(be_equal_to("v4-semver".to_string()));
    expect!(resolved_core_key("not-empty")).to(be_equal_to("v4-not-empty".to_string()));

    // Only the whole name after the version prefix counts, so these are distinct rules and not
    // ambiguous with `type`/`equals-ignore-order`
    expect!(resolved_core_key("content-type")).to(be_equal_to("v3-content-type".to_string()));
    expect!(resolved_core_key("min-type")).to(be_equal_to("v2-min-type".to_string()));
    expect!(resolved_core_key("equals-ignore-order"))
      .to(be_equal_to("v4-equals-ignore-order".to_string()));

    // The versioned key itself still resolves, by name
    expect!(resolved_core_key("v2-type")).to(be_equal_to("v2-type".to_string()));
    expect!(resolved_core_key("core/matcher/v3-date")).to(be_equal_to("v3-date".to_string()));
    expect!(resolved_core_key("matcher/v3-date")).to(be_equal_to("v3-date".to_string()));
  }

  #[test]
  fn resolve_capability_does_not_match_a_key_component_as_a_substring() {
    // "type" must not name `core/matcher/v2-type` by suffix - if it did, it would match all eight
    // core keys ending in "type" and be ambiguous. It resolves through the versioned fallback to
    // exactly one entry instead.
    expect!(names_catalogue_key("core/matcher/v2-type", "type")).to(be_false());
    expect!(names_catalogue_key("core/matcher/v2-type", "v2-type")).to(be_true());
    expect!(names_catalogue_key("core/matcher/v2-type", "matcher/v2-type")).to(be_true());
    expect!(names_catalogue_key("core/matcher/v2-type", "core/matcher/v2-type")).to(be_true());
    expect!(names_catalogue_key("core/matcher/v2-type", "r/v2-type")).to(be_false());
    expect!(names_catalogue_key("core/content-matcher/xml", "xml")).to(be_true());
    expect!(names_catalogue_key("core/content-matcher/xml", "ml")).to(be_false());
  }

  #[test]
  fn the_versioned_fallback_only_applies_to_matcher_and_generator_entries() {
    // Content matchers, content generators and transports are registered under plain names, so a
    // leading "v<n>-" there is part of the name, not a version to be stripped.
    let name = "the_versioned_fallback_only_applies_to_matcher_and_generator_entries";
    register_core_entries(&vec![
      CatalogueEntry {
        entry_type: CatalogueEntryType::CONTENT_MATCHER,
        provider_type: CatalogueEntryProviderType::CORE,
        plugin: None,
        key: format!("v2-{}", name),
        values: hashmap!{}
      },
      CatalogueEntry {
        entry_type: CatalogueEntryType::MATCHER,
        provider_type: CatalogueEntryProviderType::CORE,
        plugin: None,
        key: format!("v2-matcher-{}", name),
        values: hashmap!{}
      }
    ]);

    // The content matcher is only reachable by its actual name
    expect!(resolve_capability(name, CatalogueEntryType::CONTENT_MATCHER).is_err()).to(be_true());
    expect!(lookup_entry(name).map(|entry| entry.key)).to(be_none());
    expect!(resolve_capability(&format!("v2-{}", name), CatalogueEntryType::CONTENT_MATCHER).is_ok())
      .to(be_true());

    // ... while the matcher entry still gets the fallback
    expect!(resolved_core_key(&format!("matcher-{}", name)))
      .to(be_equal_to(format!("v2-matcher-{}", name)));
  }

  #[test]
  fn lookup_entry_matches_by_name_not_by_substring() {
    let name = "lookup_entry_matches_by_name_not_by_substring";
    let manifest = PactPluginManifest { name: name.to_string(), .. PactPluginManifest::default() };
    register_plugin_entries(&manifest, &vec![
      ProtoCatalogueEntry {
        r#type: CatalogueEntryType::CONTENT_MATCHER.to_proto_value(),
        key: name.to_string(),
        values: hashmap!{}
      },
      ProtoCatalogueEntry {
        r#type: CatalogueEntryType::MATCHER.to_proto_value(),
        key: format!("v3-{}", name),
        values: hashmap!{}
      }
    ]);

    let by_name = lookup_entry(name).map(|entry| entry.entry_type);
    let by_components = lookup_entry(&format!("content-matcher/{}", name)).map(|entry| entry.entry_type);
    let fully_qualified = lookup_entry(&format!("plugin/{}/content-matcher/{}", name, name))
      .map(|entry| entry.entry_type);
    // A trailing substring of a component names nothing
    let by_substring = lookup_entry(&name[3..]);
    // But the versioned convention still resolves for a matcher entry
    let versioned = lookup_entry(&format!("{}-{}", "matcher-fallback", name));

    remove_plugin_entries(name);

    expect!(by_name).to(be_some().value(CatalogueEntryType::CONTENT_MATCHER));
    expect!(by_components).to(be_some().value(CatalogueEntryType::CONTENT_MATCHER));
    expect!(fully_qualified).to(be_some().value(CatalogueEntryType::CONTENT_MATCHER));
    expect!(by_substring).to(be_none());
    expect!(versioned).to(be_none());
  }

  #[test]
  fn lookup_entry_falls_back_to_the_versioned_core_key() {
    register_core_matcher_entries();

    expect!(lookup_entry("type").map(|entry| entry.key)).to(be_some().value("v2-type".to_string()));
    expect!(lookup_entry("v3-date").map(|entry| entry.key)).to(be_some().value("v3-date".to_string()));
    expect!(lookup_entry("matcher/v3-date").map(|entry| entry.key)).to(be_some().value("v3-date".to_string()));
  }

  #[test]
  fn resolve_capability_prefers_an_entry_named_directly_over_the_versioned_fallback() {
    let name = "resolve_capability_prefers_an_entry_named_directly_over_the_versioned_fallback";
    register_core_entries(&vec![CatalogueEntry {
      entry_type: CatalogueEntryType::MATCHER,
      provider_type: CatalogueEntryProviderType::CORE,
      plugin: None,
      key: format!("v3-{}", name),
      values: hashmap!{}
    }]);
    // Before the plugin registers anything, the bare name finds the core rule via the fallback
    let core_first = resolved_core_key(name);

    let manifest = PactPluginManifest { name: name.to_string(), .. PactPluginManifest::default() };
    register_plugin_entries(&manifest, &vec![ProtoCatalogueEntry {
      r#type: CatalogueEntryType::MATCHER.to_proto_value(),
      key: name.to_string(),
      values: hashmap!{}
    }]);

    let resolved = resolve_capability(name, CatalogueEntryType::MATCHER);
    // A caller that specifically wants the core rule can still name its versioned key
    let still_core = resolved_core_key(&format!("v3-{}", name));
    remove_plugin_entries(name);

    expect!(core_first).to(be_equal_to(format!("v3-{}", name)));
    match resolved.expect("expected the plugin's own entry to resolve") {
      ResolvedCapability::Plugin(resolved_manifest) => expect!(resolved_manifest.name).to(be_equal_to(name.to_string())),
      ResolvedCapability::Core(key) => panic!("expected the plugin entry to win, got core '{}'", key)
    };
    expect!(still_core).to(be_equal_to(format!("v3-{}", name)));
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
