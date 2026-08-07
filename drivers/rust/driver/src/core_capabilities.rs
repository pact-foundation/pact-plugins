//! Registry of host-provided ("core") capability handlers, keyed by catalogue entry key.
//!
//! This generalises the [`crate::plugin_log_sink::PluginLogSink`] pattern from a single sink to
//! one handler per capability shape: the driver defines a narrow trait per capability (matching
//! an operation already defined for plugins), the embedding Pact framework implements it and
//! registers an instance here at startup, and the driver never has a compile-time dependency on
//! that implementation. See proposal 007 (Driver-plugin callback model) for the full design.
//!
//! Registration should happen alongside [`crate::catalogue_manager::register_core_entries`] for
//! the corresponding `CatalogueEntryProviderType::CORE` entry, so an entry and its handler never
//! drift apart. Callers resolve a capability via the catalogue entry's `key` (unprefixed, e.g.
//! `"xml"` for `core/content-matcher/xml`), not the full catalogue key.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lazy_static::lazy_static;

use crate::proto::{CompareContentsRequest, CompareContentsResponse, GenerateContentRequest, GenerateContentResponse};
use crate::proto_v2::{GenerateFieldRequest, GenerateFieldResponse, MatchFieldRequest, MatchFieldResponse};

/// A host-provided handler for the `CompareContents` capability shape. Implemented by the
/// embedding Pact framework and registered via [`register_core_content_matcher`].
#[async_trait]
pub trait CoreContentMatcher: Send + Sync {
  /// Compare the actual contents against the expected contents, returning any mismatches.
  async fn compare_contents(&self, request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse>;
}

/// A host-provided handler for the `MatchField` capability shape - one of the standard Pact
/// matching rules applied to a single value. Implemented by the embedding Pact framework and
/// registered via [`register_core_field_matcher`]. See proposals 006 (Field-level matchers and
/// generators) and 009 (Host-provided core matching and generation).
///
/// The request and response are the V2 interface types: field-level operations were introduced in
/// V2 and have no V1 equivalent.
#[async_trait]
pub trait CoreFieldMatcher: Send + Sync {
  /// Apply the matching rule to a single value, returning any mismatches.
  async fn match_field(&self, request: MatchFieldRequest) -> anyhow::Result<MatchFieldResponse>;
}

/// A host-provided handler for the `GenerateField` capability shape - one of the standard Pact
/// generators applied to a single value. Implemented by the embedding Pact framework and registered
/// via [`register_core_field_generator`]. See [`CoreFieldMatcher`].
#[async_trait]
pub trait CoreFieldGenerator: Send + Sync {
  /// Generate a single value, replacing the example value from the Pact interaction.
  async fn generate_field(&self, request: GenerateFieldRequest) -> anyhow::Result<GenerateFieldResponse>;
}

/// A host-provided handler for the `GenerateContent` capability shape. Implemented by the
/// embedding Pact framework and registered via [`register_core_content_generator`].
#[async_trait]
pub trait CoreContentGenerator: Send + Sync {
  /// Generate contents using the provided generators.
  async fn generate_content(&self, request: GenerateContentRequest) -> anyhow::Result<GenerateContentResponse>;
}

lazy_static! {
  static ref CORE_CONTENT_MATCHERS: Mutex<HashMap<String, Arc<dyn CoreContentMatcher>>> = Mutex::new(HashMap::new());
  static ref CORE_CONTENT_GENERATORS: Mutex<HashMap<String, Arc<dyn CoreContentGenerator>>> = Mutex::new(HashMap::new());
  static ref CORE_FIELD_MATCHERS: Mutex<HashMap<String, Arc<dyn CoreFieldMatcher>>> = Mutex::new(HashMap::new());
  static ref CORE_FIELD_GENERATORS: Mutex<HashMap<String, Arc<dyn CoreFieldGenerator>>> = Mutex::new(HashMap::new());
}

/// Register a handler for a host-provided content matcher capability, keyed by the catalogue
/// entry key (e.g. `"xml"` for the `core/content-matcher/xml` entry). Replaces any handler
/// previously registered under the same key.
pub fn register_core_content_matcher(key: &str, handler: Arc<dyn CoreContentMatcher>) {
  CORE_CONTENT_MATCHERS.lock()
    .expect("CORE_CONTENT_MATCHERS mutex poisoned")
    .insert(key.to_string(), handler);
}

/// Register a handler for a host-provided content generator capability, keyed by the catalogue
/// entry key (e.g. `"xml"` for the `core/content-generator/xml` entry). Replaces any handler
/// previously registered under the same key.
pub fn register_core_content_generator(key: &str, handler: Arc<dyn CoreContentGenerator>) {
  CORE_CONTENT_GENERATORS.lock()
    .expect("CORE_CONTENT_GENERATORS mutex poisoned")
    .insert(key.to_string(), handler);
}

/// Look up a registered core content matcher handler by catalogue entry key.
pub fn lookup_core_content_matcher(key: &str) -> Option<Arc<dyn CoreContentMatcher>> {
  CORE_CONTENT_MATCHERS.lock()
    .expect("CORE_CONTENT_MATCHERS mutex poisoned")
    .get(key).cloned()
}

/// Look up a registered core content generator handler by catalogue entry key.
pub fn lookup_core_content_generator(key: &str) -> Option<Arc<dyn CoreContentGenerator>> {
  CORE_CONTENT_GENERATORS.lock()
    .expect("CORE_CONTENT_GENERATORS mutex poisoned")
    .get(key).cloned()
}

/// Register a handler for a host-provided field matching rule, keyed by the catalogue entry key
/// (e.g. `"type"` for the `core/matcher/type` entry). Replaces any handler previously
/// registered under the same key.
pub fn register_core_field_matcher(key: &str, handler: Arc<dyn CoreFieldMatcher>) {
  CORE_FIELD_MATCHERS.lock()
    .expect("CORE_FIELD_MATCHERS mutex poisoned")
    .insert(key.to_string(), handler);
}

/// Register a handler for a host-provided field generator, keyed by the catalogue entry key
/// (e.g. `"date"` for the `core/generator/date` entry). Replaces any handler previously
/// registered under the same key.
pub fn register_core_field_generator(key: &str, handler: Arc<dyn CoreFieldGenerator>) {
  CORE_FIELD_GENERATORS.lock()
    .expect("CORE_FIELD_GENERATORS mutex poisoned")
    .insert(key.to_string(), handler);
}

/// Look up a registered core field matcher handler by catalogue entry key.
pub fn lookup_core_field_matcher(key: &str) -> Option<Arc<dyn CoreFieldMatcher>> {
  CORE_FIELD_MATCHERS.lock()
    .expect("CORE_FIELD_MATCHERS mutex poisoned")
    .get(key).cloned()
}

/// Look up a registered core field generator handler by catalogue entry key.
pub fn lookup_core_field_generator(key: &str) -> Option<Arc<dyn CoreFieldGenerator>> {
  CORE_FIELD_GENERATORS.lock()
    .expect("CORE_FIELD_GENERATORS mutex poisoned")
    .get(key).cloned()
}

/// Remove a registered core field matcher handler. Mainly useful for tests.
pub fn deregister_core_field_matcher(key: &str) {
  CORE_FIELD_MATCHERS.lock()
    .expect("CORE_FIELD_MATCHERS mutex poisoned")
    .remove(key);
}

/// Remove a registered core field generator handler. Mainly useful for tests.
pub fn deregister_core_field_generator(key: &str) {
  CORE_FIELD_GENERATORS.lock()
    .expect("CORE_FIELD_GENERATORS mutex poisoned")
    .remove(key);
}

/// Remove a registered core content matcher handler. Mainly useful for tests.
pub fn deregister_core_content_matcher(key: &str) {
  CORE_CONTENT_MATCHERS.lock()
    .expect("CORE_CONTENT_MATCHERS mutex poisoned")
    .remove(key);
}

/// Remove a registered core content generator handler. Mainly useful for tests.
pub fn deregister_core_content_generator(key: &str) {
  CORE_CONTENT_GENERATORS.lock()
    .expect("CORE_CONTENT_GENERATORS mutex poisoned")
    .remove(key);
}

#[cfg(test)]
mod tests {
  use expectest::prelude::*;

  use crate::proto::{CompareContentsRequest, CompareContentsResponse, GenerateContentRequest, GenerateContentResponse};
  use crate::proto_v2::{GenerateFieldRequest, GenerateFieldResponse, MatchFieldRequest, MatchFieldResponse};

  use super::*;

  #[derive(Debug)]
  struct TestMatcher;

  #[async_trait]
  impl CoreContentMatcher for TestMatcher {
    async fn compare_contents(&self, _request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse> {
      Ok(CompareContentsResponse::default())
    }
  }

  #[derive(Debug)]
  struct TestGenerator;

  #[async_trait]
  impl CoreContentGenerator for TestGenerator {
    async fn generate_content(&self, _request: GenerateContentRequest) -> anyhow::Result<GenerateContentResponse> {
      Ok(GenerateContentResponse::default())
    }
  }

  #[test_log::test]
  fn returns_none_for_an_unregistered_key() {
    expect!(lookup_core_content_matcher("unregistered-matcher-key").is_none()).to(be_true());
    expect!(lookup_core_content_generator("unregistered-generator-key").is_none()).to(be_true());
  }

  #[test_log::test(tokio::test)]
  async fn registers_and_looks_up_a_content_matcher() {
    register_core_content_matcher("test-matcher-key", Arc::new(TestMatcher));

    let handler = lookup_core_content_matcher("test-matcher-key");
    deregister_core_content_matcher("test-matcher-key");

    expect!(handler.is_some()).to(be_true());
    let response = handler.unwrap().compare_contents(CompareContentsRequest::default()).await;
    expect!(response.is_ok()).to(be_true());
  }

  #[test_log::test(tokio::test)]
  async fn registers_and_looks_up_a_content_generator() {
    register_core_content_generator("test-generator-key", Arc::new(TestGenerator));

    let handler = lookup_core_content_generator("test-generator-key");
    deregister_core_content_generator("test-generator-key");

    expect!(handler.is_some()).to(be_true());
    let response = handler.unwrap().generate_content(GenerateContentRequest::default()).await;
    expect!(response.is_ok()).to(be_true());
  }

  #[test_log::test]
  fn deregister_is_a_no_op_for_an_unknown_key() {
    deregister_core_content_matcher("never-registered");
    deregister_core_content_generator("never-registered");
    deregister_core_field_matcher("never-registered");
    deregister_core_field_generator("never-registered");
  }

  #[derive(Debug)]
  struct TestFieldMatcher;

  #[async_trait]
  impl CoreFieldMatcher for TestFieldMatcher {
    async fn match_field(&self, request: MatchFieldRequest) -> anyhow::Result<MatchFieldResponse> {
      Ok(MatchFieldResponse { error: request.key, .. MatchFieldResponse::default() })
    }
  }

  #[derive(Debug)]
  struct TestFieldGenerator;

  #[async_trait]
  impl CoreFieldGenerator for TestFieldGenerator {
    async fn generate_field(&self, request: GenerateFieldRequest) -> anyhow::Result<GenerateFieldResponse> {
      Ok(GenerateFieldResponse { error: request.key, .. GenerateFieldResponse::default() })
    }
  }

  #[test_log::test]
  fn returns_none_for_an_unregistered_field_key() {
    expect!(lookup_core_field_matcher("unregistered-field-matcher-key").is_none()).to(be_true());
    expect!(lookup_core_field_generator("unregistered-field-generator-key").is_none()).to(be_true());
  }

  #[test_log::test(tokio::test)]
  async fn registers_and_looks_up_a_field_matcher() {
    register_core_field_matcher("test-field-matcher-key", Arc::new(TestFieldMatcher));

    let handler = lookup_core_field_matcher("test-field-matcher-key");
    deregister_core_field_matcher("test-field-matcher-key");

    expect!(handler.is_some()).to(be_true());
    let response = handler.unwrap()
      .match_field(MatchFieldRequest { key: "type".to_string(), .. MatchFieldRequest::default() })
      .await;
    // The stub echoes the request key back, so this also proves the request reached the handler
    expect!(response.unwrap().error).to(be_equal_to("type".to_string()));
  }

  #[test_log::test(tokio::test)]
  async fn registers_and_looks_up_a_field_generator() {
    register_core_field_generator("test-field-generator-key", Arc::new(TestFieldGenerator));

    let handler = lookup_core_field_generator("test-field-generator-key");
    deregister_core_field_generator("test-field-generator-key");

    expect!(handler.is_some()).to(be_true());
    let response = handler.unwrap()
      .generate_field(GenerateFieldRequest { key: "date".to_string(), .. GenerateFieldRequest::default() })
      .await;
    expect!(response.unwrap().error).to(be_equal_to("date".to_string()));
  }
}
