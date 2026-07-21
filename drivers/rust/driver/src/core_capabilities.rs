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

/// A host-provided handler for the `CompareContents` capability shape. Implemented by the
/// embedding Pact framework and registered via [`register_core_content_matcher`].
#[async_trait]
pub trait CoreContentMatcher: Send + Sync {
  /// Compare the actual contents against the expected contents, returning any mismatches.
  async fn compare_contents(&self, request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse>;
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
  }
}
