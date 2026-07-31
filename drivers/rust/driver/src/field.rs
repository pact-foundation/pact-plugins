//! Support for matching and generating individual field/element values.
//!
//! This is the field-level counterpart of [`crate::content`]: where a content matcher owns a whole
//! content type, a field matcher applies one matching rule to one value inside somebody else's
//! content - a field in a JSON body, a header, a message metadata value. See proposal 006
//! (Field-level matchers and generators) for the design.
//!
//! The proto types used here are the V2 interface ones. Field-level operations were introduced in
//! V2 and have no V1 equivalent, so a V1 plugin cannot provide them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use bytes::Bytes;
use lazy_static::lazy_static;
use pact_models::matchingrules::MatchingRule;
use pact_models::path_exp::DocPath;
use pact_models::prelude::Generator;
use serde_json::Value;
use tokio::runtime::Runtime;
use tracing::{debug, error};

use crate::catalogue_manager::{CatalogueEntry, CatalogueEntryProviderType, CatalogueEntryType, resolve_capability_entry};
use crate::content::ContentMismatch;
use crate::core_capabilities;
use crate::plugin_manager::lookup_plugin;
use crate::plugin_models::{PactPluginManifest, PluginInteractionConfig};
use crate::proto_v2::{
  FieldValue as ProtoFieldValue,
  GenerateFieldRequest,
  GenerateFieldResponse,
  MatchFieldRequest,
  MatchFieldResponse,
  MatchingRule as ProtoMatchingRule,
  Generator as ProtoGenerator,
  PluginConfiguration as ProtoPluginConfiguration,
  field_value
};
use crate::utils::{proto_value_to_json, to_proto_struct, to_proto_value};

/// A single value being matched or generated at the field/element level - the driver-side
/// counterpart of the proto `FieldValue`.
///
/// Binary-safe by construction: a value that is not representable as text is carried as bytes
/// rather than being stringified into one. Scalar types survive the trip to a plugin intact,
/// including the difference between a whole number and a decimal, which the `integer`, `decimal`
/// and `type` matching rules all depend on - see [`FieldValue::to_proto`].
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
  /// A JSON-like value
  Json(Value),
  /// Raw bytes, for a value that is not representable as JSON
  Binary(Bytes)
}

impl FieldValue {
  /// Convert to the protobuf form sent to a plugin or core handler.
  ///
  /// Each scalar type gets its own arm, so a matching rule on the other side sees the type the
  /// value actually has - in particular a whole number stays whole, which `integer`, `decimal` and
  /// `type` all depend on. Only maps and lists go across as a `google.protobuf.Value`.
  pub fn to_proto(&self) -> ProtoFieldValue {
    ProtoFieldValue {
      value: Some(match self {
        FieldValue::Binary(bytes) => field_value::Value::BinaryValue(bytes.to_vec()),
        FieldValue::Json(Value::Null) => field_value::Value::NullValue(0),
        FieldValue::Json(Value::Bool(value)) => field_value::Value::BooleanValue(*value),
        FieldValue::Json(Value::String(value)) => field_value::Value::StringValue(value.clone()),
        FieldValue::Json(Value::Number(number)) => match number.as_i64() {
          Some(value) => field_value::Value::IntegerValue(value),
          // A u64 above i64::MAX has nowhere exact to go; a double at least keeps the magnitude
          None => field_value::Value::DecimalValue(number.as_f64().unwrap_or_default())
        },
        FieldValue::Json(value) => field_value::Value::StructuredValue(to_proto_value(value))
      })
    }
  }

  /// Convert from the protobuf form returned by a plugin or core handler. An unset value is
  /// treated as null, matching how an absent `oneof` reads everywhere else in the interface.
  pub fn from_proto(value: &ProtoFieldValue) -> FieldValue {
    match &value.value {
      Some(field_value::Value::NullValue(_)) | None => FieldValue::Json(Value::Null),
      Some(field_value::Value::BooleanValue(value)) => FieldValue::Json(Value::Bool(*value)),
      Some(field_value::Value::StringValue(value)) => FieldValue::Json(Value::String(value.clone())),
      Some(field_value::Value::IntegerValue(value)) => FieldValue::Json(Value::Number((*value).into())),
      Some(field_value::Value::DecimalValue(value)) => FieldValue::Json(
        serde_json::Number::from_f64(*value)
          .map(Value::Number)
          // NaN and the infinities have no JSON representation
          .unwrap_or(Value::Null)
      ),
      Some(field_value::Value::BinaryValue(bytes)) => FieldValue::Binary(Bytes::from(bytes.clone())),
      Some(field_value::Value::StructuredValue(value)) => FieldValue::Json(proto_value_to_json(value))
    }
  }
}

impl From<Value> for FieldValue {
  fn from(value: Value) -> Self {
    FieldValue::Json(value)
  }
}

impl From<Bytes> for FieldValue {
  fn from(bytes: Bytes) -> Self {
    FieldValue::Binary(bytes)
  }
}

/// Where a value sits and what is known about it, shared by matching and generation.
#[derive(Clone, Debug)]
pub struct FieldContext {
  /// Path to the value, as a Pact matching rule expression (`$.card.number`)
  pub path: DocPath,
  /// Part of the interaction the value came from: `body`, `header`, `metadata`, `query`, `path`,
  /// `status`. Only affects how a mismatch is reported; generation ignores it.
  pub category: String,
  /// Plugin configuration persisted into the Pact file for this interaction
  pub plugin_config: Option<PluginInteractionConfig>,
  /// Context data provided by the test framework
  pub test_context: HashMap<String, Value>
}

impl Default for FieldContext {
  fn default() -> Self {
    FieldContext {
      path: DocPath::root(),
      category: "body".to_string(),
      plugin_config: None,
      test_context: HashMap::default()
    }
  }
}

impl FieldContext {
  /// A context for a value at the given path in the given part of the interaction
  pub fn new(path: &DocPath, category: &str) -> FieldContext {
    FieldContext {
      path: path.clone(),
      category: category.to_string(),
      .. FieldContext::default()
    }
  }

  /// Set the plugin configuration
  pub fn with_plugin_config(self, plugin_config: Option<PluginInteractionConfig>) -> FieldContext {
    FieldContext { plugin_config, .. self }
  }

  /// Set the test framework context data
  pub fn with_test_context(self, test_context: HashMap<String, Value>) -> FieldContext {
    FieldContext { test_context, .. self }
  }
}

/// Matching rule for a single field/element value, provided by a plugin or by a handler the host
/// framework registered (see [`crate::core_capabilities::CoreFieldMatcher`]).
#[derive(Clone, Debug)]
pub struct FieldMatcher {
  /// Catalogue entry for this matching rule
  pub catalogue_entry: CatalogueEntry
}

/// Generator for a single field/element value. See [`FieldMatcher`].
#[derive(Clone, Debug)]
pub struct FieldGenerator {
  /// Catalogue entry for this generator
  pub catalogue_entry: CatalogueEntry
}

/// Find the field-level matching rule with the given name. The name is resolved against the
/// catalogue the same way any other capability key is - see
/// [`crate::catalogue_manager::resolve_capability`] - so `creditcard` finds a plugin's own rule and
/// `type` finds the core `v2-type` rule. Returns a descriptive error if the name matches nothing,
/// matches more than one rule, or names something that is not a matching rule.
pub fn find_field_matcher(name: &str) -> anyhow::Result<FieldMatcher> {
  resolve_capability_entry(name, CatalogueEntryType::MATCHER)
    .map(|catalogue_entry| FieldMatcher { catalogue_entry })
}

/// Find the field-level generator with the given name. See [`find_field_matcher`].
pub fn find_field_generator(name: &str) -> anyhow::Result<FieldGenerator> {
  resolve_capability_entry(name, CatalogueEntryType::GENERATOR)
    .map(|catalogue_entry| FieldGenerator { catalogue_entry })
}

impl FieldMatcher {
  /// If this is a matching rule provided by the core framework rather than a plugin
  pub fn is_core(&self) -> bool {
    self.catalogue_entry.provider_type == CatalogueEntryProviderType::CORE
  }

  /// Catalogue entry key for this matching rule
  pub fn catalogue_entry_key(&self) -> String {
    if self.is_core() {
      format!("core/matcher/{}", self.catalogue_entry.key)
    } else {
      format!("plugin/{}/matcher/{}", self.plugin_name(), self.catalogue_entry.key)
    }
  }

  /// Plugin that provides this matching rule, if any
  pub fn plugin(&self) -> Option<PactPluginManifest> {
    self.catalogue_entry.plugin.clone()
  }

  /// Name of the plugin that provides this matching rule
  pub fn plugin_name(&self) -> String {
    self.catalogue_entry.plugin.as_ref()
      .map(|plugin| plugin.name.clone())
      .unwrap_or("core".to_string())
  }

  /// Apply this matching rule to a single value.
  ///
  /// The context carries where the value lives and which part of the interaction it came from;
  /// both are echoed back on any mismatch that does not place itself. An empty result means the
  /// value matched.
  pub async fn match_field(
    &self,
    rule: &MatchingRule,
    expected: &FieldValue,
    actual: &FieldValue,
    context: &FieldContext
  ) -> Result<(), Vec<ContentMismatch>> {
    let request = MatchFieldRequest {
      key: self.catalogue_entry.key.clone(),
      rule: Some(to_proto_matching_rule(rule)),
      path: context.path.to_string(),
      mismatch_type: context.category.clone(),
      expected: Some(expected.to_proto()),
      actual: Some(actual.to_proto()),
      plugin_configuration: context.plugin_config.clone().map(to_proto_plugin_config),
      test_context: Some(to_proto_struct(&context.test_context))
    };

    let response = if self.is_core() {
      match core_capabilities::lookup_core_field_matcher(&self.catalogue_entry.key) {
        Some(handler) => handler.match_field(request).await,
        None => Err(anyhow!("No core field matcher registered for '{}'", self.catalogue_entry.key))
      }
    } else {
      self.call_plugin(request).await
    };

    process_match_field_response(response, context)
  }

  async fn call_plugin(&self, request: MatchFieldRequest) -> anyhow::Result<MatchFieldResponse> {
    let manifest = self.catalogue_entry.plugin.as_ref()
      .ok_or_else(|| anyhow!("Catalogue entry '{}' has no plugin manifest", self.catalogue_entry_key()))?;
    let plugin = lookup_plugin(&manifest.as_dependency())
      .ok_or_else(|| anyhow!("Plugin '{}' for matching rule '{}' is not currently running",
        manifest.name, self.catalogue_entry.key))?;
    debug!("Sending MatchField request to plugin {:?}", manifest.name);
    let chain_id = crate::call_chain::new_call_chain_id();
    let deadline_ms = crate::call_chain::default_deadline_ms();
    plugin.match_field_with_chain(request, &chain_id, deadline_ms).await
  }

  /// Apply this matching rule to a single value from a synchronous call path.
  ///
  /// Every Rust host applies matching rules synchronously (`match_values` and the matching engine's
  /// `execute_*_plan` functions), while a plugin call is async, so this bridge exists so each host
  /// does not have to build its own - see [`block_on_field_call`] for why the obvious ways of doing
  /// it do not work.
  pub fn match_field_blocking(
    &self,
    rule: &MatchingRule,
    expected: &FieldValue,
    actual: &FieldValue,
    context: &FieldContext
  ) -> Result<(), Vec<ContentMismatch>> {
    let matcher = self.clone();
    let rule = rule.clone();
    let expected = expected.clone();
    let actual = actual.clone();
    let call_context = context.clone();

    block_on_field_call(async move {
      matcher.match_field(&rule, &expected, &actual, &call_context).await
    })
    .unwrap_or_else(|err| Err(vec![mismatch_for(err.to_string(), context)]))
  }
}

impl FieldGenerator {
  /// If this is a generator provided by the core framework rather than a plugin
  pub fn is_core(&self) -> bool {
    self.catalogue_entry.provider_type == CatalogueEntryProviderType::CORE
  }

  /// Catalogue entry key for this generator
  pub fn catalogue_entry_key(&self) -> String {
    if self.is_core() {
      format!("core/generator/{}", self.catalogue_entry.key)
    } else {
      format!("plugin/{}/generator/{}", self.plugin_name(), self.catalogue_entry.key)
    }
  }

  /// Plugin that provides this generator, if any
  pub fn plugin(&self) -> Option<PactPluginManifest> {
    self.catalogue_entry.plugin.clone()
  }

  /// Name of the plugin that provides this generator
  pub fn plugin_name(&self) -> String {
    self.catalogue_entry.plugin.as_ref()
      .map(|plugin| plugin.name.clone())
      .unwrap_or("core".to_string())
  }

  /// Generate a single value, replacing the example value from the Pact interaction.
  pub async fn generate_field(
    &self,
    generator: &Generator,
    example: &FieldValue,
    mode: TestMode,
    context: &FieldContext
  ) -> anyhow::Result<FieldValue> {
    let request = GenerateFieldRequest {
      key: self.catalogue_entry.key.clone(),
      generator: Some(to_proto_generator(generator)),
      path: context.path.to_string(),
      example_value: Some(example.to_proto()),
      plugin_configuration: context.plugin_config.clone().map(to_proto_plugin_config),
      test_context: Some(to_proto_struct(&context.test_context)),
      test_mode: mode.to_proto() as i32
    };

    let response = if self.is_core() {
      let handler = core_capabilities::lookup_core_field_generator(&self.catalogue_entry.key)
        .ok_or_else(|| anyhow!("No core field generator registered for '{}'", self.catalogue_entry.key))?;
      handler.generate_field(request).await?
    } else {
      self.call_plugin(request).await?
    };

    if !response.error.is_empty() {
      return Err(anyhow!("Generator '{}' failed: {}", self.catalogue_entry.key, response.error));
    }
    match &response.value {
      Some(value) => Ok(FieldValue::from_proto(value)),
      None => Err(anyhow!("Generator '{}' returned no value", self.catalogue_entry.key))
    }
  }

  async fn call_plugin(&self, request: GenerateFieldRequest) -> anyhow::Result<GenerateFieldResponse> {
    let manifest = self.catalogue_entry.plugin.as_ref()
      .ok_or_else(|| anyhow!("Catalogue entry '{}' has no plugin manifest", self.catalogue_entry_key()))?;
    let plugin = lookup_plugin(&manifest.as_dependency())
      .ok_or_else(|| anyhow!("Plugin '{}' for generator '{}' is not currently running",
        manifest.name, self.catalogue_entry.key))?;
    debug!("Sending GenerateField request to plugin {:?}", manifest.name);
    let chain_id = crate::call_chain::new_call_chain_id();
    let deadline_ms = crate::call_chain::default_deadline_ms();
    plugin.generate_field_with_chain(request, &chain_id, deadline_ms).await
  }

  /// Generate a single value from a synchronous call path. See
  /// [`FieldMatcher::match_field_blocking`].
  pub fn generate_field_blocking(
    &self,
    generator: &Generator,
    example: &FieldValue,
    mode: TestMode,
    context: &FieldContext
  ) -> anyhow::Result<FieldValue> {
    let field_generator = self.clone();
    let generator = generator.clone();
    let example = example.clone();
    let context = context.clone();

    block_on_field_call(async move {
      field_generator.generate_field(&generator, &example, mode, &context).await
    })?
  }
}

/// Which side of the test a generator is running on, mirroring `GenerateContentRequest.TestMode`
/// in the plugin interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestMode {
  /// Running on the consumer side
  Consumer,
  /// Running on the provider side
  Provider,
  /// Not known
  Unknown
}

impl TestMode {
  fn to_proto(self) -> crate::proto_v2::generate_content_request::TestMode {
    use crate::proto_v2::generate_content_request::TestMode as ProtoTestMode;
    match self {
      TestMode::Consumer => ProtoTestMode::Consumer,
      TestMode::Provider => ProtoTestMode::Provider,
      TestMode::Unknown => ProtoTestMode::Unknown
    }
  }
}

lazy_static! {
  /// Runtime the driver owns for field-level plugin calls made from a synchronous call path.
  /// Built on first use and never dropped - dropping a Tokio runtime from inside an async context
  /// panics, and by definition this one is reached from call paths that may be exactly that.
  static ref FIELD_RUNTIME: Mutex<Option<Arc<Runtime>>> = Mutex::new(None);
}

fn field_runtime() -> anyhow::Result<Arc<Runtime>> {
  let mut guard = FIELD_RUNTIME.lock()
    .map_err(|err| anyhow!("FIELD_RUNTIME mutex poisoned - {}", err))?;
  match guard.as_ref() {
    Some(runtime) => Ok(runtime.clone()),
    None => {
      let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .thread_name("pact-plugin-field")
        .build()?);
      *guard = Some(runtime.clone());
      Ok(runtime)
    }
  }
}

/// Run a field-level plugin call to completion from a synchronous call path.
///
/// The obvious approaches do not work. `Handle::current().block_on(..)` panics ("Cannot start a
/// runtime from within a runtime") because matching is reached from an async call path, so the
/// calling thread is already driving tasks. `task::block_in_place` re-enters legitimately but
/// panics on a `current_thread` runtime, and some Pact entry points use one.
///
/// So the future runs on a runtime the driver owns, and the calling thread waits on a channel. It
/// does block a host thread for the duration, which is inherent to bridging sync and async, but the
/// plugin call itself never depends on the host's runtime making progress: the driver opens a fresh
/// gRPC channel per call (see `GrpcPactPlugin::connect_channel`), so the connection driving that
/// call belongs to this runtime too.
fn block_on_field_call<F, T>(future: F) -> anyhow::Result<T>
where
  F: std::future::Future<Output = T> + Send + 'static,
  T: Send + 'static
{
  let runtime = field_runtime()?;
  let deadline_ms = crate::call_chain::default_deadline_ms();
  let (sender, receiver) = std::sync::mpsc::channel();
  runtime.spawn(async move {
    // A send error just means the caller already gave up waiting
    let _ = sender.send(future.await);
  });
  receiver.recv_timeout(crate::call_chain::remaining(deadline_ms))
    .map_err(|err| {
      error!("Timed out waiting for a field-level plugin call to complete - {}", err);
      anyhow!("Timed out waiting for the plugin call to complete - {}", err)
    })
}

fn process_match_field_response(
  response: anyhow::Result<MatchFieldResponse>,
  context: &FieldContext
) -> Result<(), Vec<ContentMismatch>> {
  let path = context.path.to_string();
  match response {
    Ok(response) => if !response.error.is_empty() {
      Err(vec![mismatch_for(response.error, context)])
    } else if response.mismatches.is_empty() {
      Ok(())
    } else {
      Err(response.mismatches.iter().map(|mismatch| ContentMismatch {
        expected: mismatch.expected.as_ref()
          .map(|bytes| String::from_utf8_lossy(bytes).to_string())
          .unwrap_or_default(),
        actual: mismatch.actual.as_ref()
          .map(|bytes| String::from_utf8_lossy(bytes).to_string())
          .unwrap_or_default(),
        mismatch: mismatch.mismatch.clone(),
        // A mismatch that does not place itself is reported against the value being matched
        path: if mismatch.path.is_empty() { path.clone() } else { mismatch.path.clone() },
        diff: if mismatch.diff.is_empty() { None } else { Some(mismatch.diff.clone()) },
        mismatch_type: if mismatch.mismatch_type.is_empty() {
          Some(context.category.clone())
        } else {
          Some(mismatch.mismatch_type.clone())
        }
      }).collect())
    },
    Err(err) => {
      error!("Field-level match call failed - {}", err);
      Err(vec![mismatch_for(err.to_string(), context)])
    }
  }
}

fn mismatch_for(message: String, context: &FieldContext) -> ContentMismatch {
  ContentMismatch {
    expected: Default::default(),
    actual: Default::default(),
    mismatch: message,
    path: context.path.to_string(),
    diff: None,
    mismatch_type: Some(context.category.clone())
  }
}

fn to_proto_matching_rule(rule: &MatchingRule) -> ProtoMatchingRule {
  ProtoMatchingRule {
    r#type: rule.name(),
    values: Some(to_proto_struct(&rule.values().iter()
      .map(|(k, v)| (k.to_string(), v.clone()))
      .collect()))
  }
}

fn to_proto_generator(generator: &Generator) -> ProtoGenerator {
  ProtoGenerator {
    r#type: generator.name(),
    values: Some(to_proto_struct(&generator.values().iter()
      .map(|(k, v)| (k.to_string(), v.clone()))
      .collect()))
  }
}

fn to_proto_plugin_config(config: PluginInteractionConfig) -> ProtoPluginConfiguration {
  ProtoPluginConfiguration {
    interaction_configuration: Some(to_proto_struct(&config.interaction_configuration)),
    pact_configuration: Some(to_proto_struct(&config.pact_configuration))
  }
}

#[cfg(test)]
mod tests {
  use async_trait::async_trait;
  use expectest::prelude::*;
  use maplit::hashmap;
  use pact_models::matchingrules::MatchingRule;

  use crate::catalogue_manager::{CatalogueEntryProviderType, register_core_entries};
  use crate::core_capabilities::{
    CoreFieldGenerator,
    CoreFieldMatcher,
    deregister_core_field_generator,
    deregister_core_field_matcher,
    register_core_field_generator,
    register_core_field_matcher
  };
  use crate::proto_v2::ContentMismatch as ProtoContentMismatch;

  use super::*;

  #[test]
  fn field_values_round_trip_through_the_proto_form() {
    for value in [
      FieldValue::Json(Value::String("4111111111111111".to_string())),
      FieldValue::Json(serde_json::json!(100)),
      FieldValue::Json(serde_json::json!(-100.5)),
      FieldValue::Json(Value::Bool(true)),
      FieldValue::Json(Value::Null),
      FieldValue::Json(serde_json::json!({ "brand": "visa" })),
      // A value that is not representable as JSON survives as bytes rather than being stringified
      FieldValue::Binary(Bytes::from(vec![0u8, 159, 146, 150]))
    ] {
      expect!(FieldValue::from_proto(&value.to_proto())).to(be_equal_to(value));
    }
  }

  #[test]
  fn a_whole_number_stays_whole_and_a_decimal_stays_decimal() {
    // The distinction the `integer`, `decimal` and `type` rules are built on, and the reason
    // FieldValue does not put every value through a google.protobuf.Value
    let integer = FieldValue::from_proto(&FieldValue::Json(serde_json::json!(100)).to_proto());
    let decimal = FieldValue::from_proto(&FieldValue::Json(serde_json::json!(100.5)).to_proto());
    let whole_decimal = FieldValue::from_proto(&FieldValue::Json(serde_json::json!(100.0)).to_proto());

    expect!(integer.clone()).to(be_equal_to(FieldValue::Json(serde_json::json!(100))));
    expect!(decimal).to(be_equal_to(FieldValue::Json(serde_json::json!(100.5))));
    match integer {
      FieldValue::Json(Value::Number(number)) => expect!(number.is_i64()).to(be_true()),
      other => panic!("expected a JSON number, got {:?}", other)
    };
    // A decimal that happens to be whole stays a decimal - it is not quietly promoted to an
    // integer, which would make `decimal` reject a value it should accept
    expect!(whole_decimal.clone()).to(be_equal_to(FieldValue::Json(serde_json::json!(100.0))));
    match whole_decimal {
      FieldValue::Json(Value::Number(number)) => expect!(number.is_f64()).to(be_true()),
      other => panic!("expected a JSON number, got {:?}", other)
    };
  }

  #[test]
  fn each_scalar_type_crosses_the_boundary_under_its_own_arm() {
    let cases = [
      (FieldValue::Json(Value::Null), "null"),
      (FieldValue::Json(Value::Bool(true)), "boolean"),
      (FieldValue::Json(serde_json::json!("4111111111111111")), "string"),
      (FieldValue::Json(serde_json::json!(100)), "integer"),
      (FieldValue::Json(serde_json::json!(100.5)), "decimal"),
      (FieldValue::Binary(Bytes::from(vec![0u8, 159, 146, 150])), "binary"),
      (FieldValue::Json(serde_json::json!({ "brand": "visa" })), "structured")
    ];
    for (value, expected_arm) in cases {
      let arm = match value.to_proto().value {
        Some(field_value::Value::NullValue(_)) => "null",
        Some(field_value::Value::BooleanValue(_)) => "boolean",
        Some(field_value::Value::StringValue(_)) => "string",
        Some(field_value::Value::IntegerValue(_)) => "integer",
        Some(field_value::Value::DecimalValue(_)) => "decimal",
        Some(field_value::Value::BinaryValue(_)) => "binary",
        Some(field_value::Value::StructuredValue(_)) => "structured",
        None => "unset"
      };
      expect!(arm).to(be_equal_to(expected_arm));
    }
  }

  #[test]
  fn an_unset_proto_value_reads_as_json_null() {
    expect!(FieldValue::from_proto(&ProtoFieldValue { value: None }))
      .to(be_equal_to(FieldValue::Json(Value::Null)));
  }

  /// Records the request it was given, and answers with the mismatches it was built with
  #[derive(Debug)]
  struct TestCoreMatcher {
    mismatches: Vec<ProtoContentMismatch>,
    error: String
  }

  #[async_trait]
  impl CoreFieldMatcher for TestCoreMatcher {
    async fn match_field(&self, request: MatchFieldRequest) -> anyhow::Result<MatchFieldResponse> {
      // Prove the request carried what the caller passed in
      assert_eq!(request.path, "$.card.number");
      assert_eq!(request.mismatch_type, "body");
      assert_eq!(request.rule.as_ref().unwrap().r#type, "regex");
      Ok(MatchFieldResponse {
        error: self.error.clone(),
        mismatches: self.mismatches.clone()
      })
    }
  }

  #[derive(Debug)]
  struct TestCoreGenerator;

  #[async_trait]
  impl CoreFieldGenerator for TestCoreGenerator {
    async fn generate_field(&self, request: GenerateFieldRequest) -> anyhow::Result<GenerateFieldResponse> {
      assert_eq!(request.path, "$.card.number");
      assert_eq!(request.test_mode, TestMode::Consumer.to_proto() as i32);
      Ok(GenerateFieldResponse {
        error: String::default(),
        value: Some(FieldValue::Json(Value::String("4012888888881881".to_string())).to_proto())
      })
    }
  }

  fn register_core_matcher_entry(key: &str, entry_type: CatalogueEntryType) {
    register_core_entries(&vec![CatalogueEntry {
      entry_type,
      provider_type: CatalogueEntryProviderType::CORE,
      plugin: None,
      key: key.to_string(),
      values: hashmap!{}
    }]);
  }

  /// The driver forwards whatever rule the host hands it - `rule.name()` and `rule.values()` -
  /// so any rule exercises the plumbing. Once pact_models grows the `Plugin` carrier variant
  /// (proposal 006 section 4), a plugin's own rule name arrives here by exactly this path.
  fn a_rule() -> MatchingRule {
    MatchingRule::Regex("\\d{16}".to_string())
  }

  fn field_context() -> FieldContext {
    FieldContext::new(&DocPath::new("$.card.number").unwrap(), "body")
  }

  #[test_log::test(tokio::test)]
  async fn match_field_dispatches_to_a_registered_core_handler() {
    let key = "match_field_dispatches_to_a_registered_core_handler";
    register_core_matcher_entry(key, CatalogueEntryType::MATCHER);
    register_core_field_matcher(key, Arc::new(TestCoreMatcher {
      mismatches: vec![],
      error: String::default()
    }));

    let matcher = find_field_matcher(key).unwrap();
    let result = matcher.match_field(
      &a_rule(),
      &FieldValue::Json(Value::String("4111111111111111".to_string())),
      &FieldValue::Json(Value::String("4012888888881881".to_string())),
      &field_context()
    ).await;

    deregister_core_field_matcher(key);

    expect!(matcher.is_core()).to(be_true());
    expect!(result).to(be_ok());
  }

  #[test_log::test(tokio::test)]
  async fn match_field_reports_mismatches_against_the_requested_path() {
    let key = "match_field_reports_mismatches_against_the_requested_path";
    register_core_matcher_entry(key, CatalogueEntryType::MATCHER);
    register_core_field_matcher(key, Arc::new(TestCoreMatcher {
      mismatches: vec![ProtoContentMismatch {
        // Deliberately no path/mismatchType: the driver fills them in from the request
        mismatch: "fails the Luhn check".to_string(),
        expected: Some("4111111111111111".as_bytes().to_vec()),
        actual: Some("4111111111111112".as_bytes().to_vec()),
        .. ProtoContentMismatch::default()
      }],
      error: String::default()
    }));

    let matcher = find_field_matcher(key).unwrap();
    let result = matcher.match_field(
      &a_rule(),
      &FieldValue::Json(Value::String("4111111111111111".to_string())),
      &FieldValue::Json(Value::String("4111111111111112".to_string())),
      &field_context()
    ).await;

    deregister_core_field_matcher(key);

    let mismatches = result.expect_err("expected a mismatch");
    expect!(mismatches.len()).to(be_equal_to(1));
    expect!(mismatches[0].mismatch.clone()).to(be_equal_to("fails the Luhn check".to_string()));
    expect!(mismatches[0].path.clone()).to(be_equal_to("$.card.number".to_string()));
    expect!(mismatches[0].mismatch_type.clone()).to(be_some().value("body".to_string()));
    expect!(mismatches[0].expected.clone()).to(be_equal_to("4111111111111111".to_string()));
  }

  #[test_log::test(tokio::test)]
  async fn match_field_turns_a_handler_error_into_a_mismatch() {
    let key = "match_field_turns_a_handler_error_into_a_mismatch";
    register_core_matcher_entry(key, CatalogueEntryType::MATCHER);
    register_core_field_matcher(key, Arc::new(TestCoreMatcher {
      mismatches: vec![],
      error: "'amx' is not a brand this plugin knows about".to_string()
    }));

    let matcher = find_field_matcher(key).unwrap();
    let result = matcher.match_field(
      &a_rule(),
      &FieldValue::Json(Value::String("4111111111111111".to_string())),
      &FieldValue::Json(Value::String("4111111111111111".to_string())),
      &field_context()
    ).await;

    deregister_core_field_matcher(key);

    let mismatches = result.expect_err("expected the error to surface");
    expect!(mismatches[0].mismatch.clone())
      .to(be_equal_to("'amx' is not a brand this plugin knows about".to_string()));
  }

  #[test_log::test(tokio::test)]
  async fn match_field_fails_clearly_when_no_core_handler_is_registered() {
    let key = "match_field_fails_clearly_when_no_core_handler_is_registered";
    register_core_matcher_entry(key, CatalogueEntryType::MATCHER);

    let matcher = find_field_matcher(key).unwrap();
    let result = matcher.match_field(
      &a_rule(),
      &FieldValue::Json(Value::Null),
      &FieldValue::Json(Value::Null),
      &field_context()
    ).await;

    let mismatches = result.expect_err("expected an error for a registered entry with no handler");
    expect!(mismatches[0].mismatch.contains("No core field matcher registered")).to(be_true());
  }

  #[test_log::test(tokio::test)]
  async fn generate_field_dispatches_to_a_registered_core_handler() {
    let key = "generate_field_dispatches_to_a_registered_core_handler";
    register_core_matcher_entry(key, CatalogueEntryType::GENERATOR);
    register_core_field_generator(key, Arc::new(TestCoreGenerator));

    let generator = find_field_generator(key).unwrap();
    let result = generator.generate_field(
      &Generator::RandomString(16),
      &FieldValue::Json(Value::String("4111111111111111".to_string())),
      TestMode::Consumer,
      &field_context()
    ).await;

    deregister_core_field_generator(key);

    expect!(generator.is_core()).to(be_true());
    expect!(result.unwrap()).to(be_equal_to(
      FieldValue::Json(Value::String("4012888888881881".to_string()))
    ));
  }

  #[test]
  fn finding_a_rule_that_is_not_registered_says_so() {
    let err = find_field_matcher("finding_a_rule_that_is_not_registered_says_so")
      .expect_err("expected an error for an unregistered rule");
    expect!(err.to_string().contains("No catalogue entry found")).to(be_true());
  }

  #[test]
  fn finding_a_rule_that_is_a_generator_says_so() {
    let key = "finding_a_rule_that_is_a_generator_says_so";
    register_core_matcher_entry(key, CatalogueEntryType::GENERATOR);

    let err = find_field_matcher(key).expect_err("expected an error for the wrong entry type");
    expect!(err.to_string().contains("is a GENERATOR, not a MATCHER")).to(be_true());
  }

  #[test_log::test]
  fn the_blocking_bridge_runs_a_call_from_a_synchronous_context() {
    let key = "the_blocking_bridge_runs_a_call_from_a_synchronous_context";
    register_core_matcher_entry(key, CatalogueEntryType::MATCHER);
    register_core_field_matcher(key, Arc::new(TestCoreMatcher {
      mismatches: vec![],
      error: String::default()
    }));

    let matcher = find_field_matcher(key).unwrap();
    let result = matcher.match_field_blocking(
      &a_rule(),
      &FieldValue::Json(Value::String("4111111111111111".to_string())),
      &FieldValue::Json(Value::String("4012888888881881".to_string())),
      &field_context()
    );

    deregister_core_field_matcher(key);

    expect!(result).to(be_ok());
  }

  #[test_log::test(tokio::test(flavor = "multi_thread"))]
  async fn the_blocking_bridge_works_from_inside_a_runtime() {
    // The case Handle::block_on panics on: the calling thread is already driving async tasks.
    // Run it on a blocking thread, which is how a host's synchronous matching path reaches us.
    let key = "the_blocking_bridge_works_from_inside_a_runtime";
    register_core_matcher_entry(key, CatalogueEntryType::MATCHER);
    register_core_field_matcher(key, Arc::new(TestCoreMatcher {
      mismatches: vec![],
      error: String::default()
    }));

    let result = tokio::task::spawn_blocking(move || {
      let matcher = find_field_matcher(key).unwrap();
      matcher.match_field_blocking(
        &a_rule(),
        &FieldValue::Json(Value::String("4111111111111111".to_string())),
        &FieldValue::Json(Value::String("4012888888881881".to_string())),
        &field_context()
      )
    }).await.unwrap();

    deregister_core_field_matcher(key);

    expect!(result).to(be_ok());
  }
}
