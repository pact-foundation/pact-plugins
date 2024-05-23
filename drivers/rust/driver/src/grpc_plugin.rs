//! Support for plugins running via the Plugin gRPC interface

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::str::from_utf8;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::anyhow;
use async_trait::async_trait;
use bytes::Bytes;
use itertools::Either;
use log::max_level;
use maplit::hashmap;
use os_info::Type;
use pact_models::bodies::OptionalBody;
use pact_models::content_types::{ContentType, ContentTypeHint};
use pact_models::generators::{Generator, GeneratorCategory, Generators};
use pact_models::matchingrules::{Category, MatchingRule, MatchingRuleCategory, RuleList, RuleLogic};
use pact_models::pact::Pact;
use pact_models::PactSpecification;
use pact_models::path_exp::DocPath;
use pact_models::prelude::v4::V4Pact;
use pact_models::v4::interaction::V4Interaction;
use serde_json::Value;
use sysinfo::{Pid, Signal, System};
use tokio::process::Command;
use tonic::{Request, Status};
use tonic::codegen::InterceptedService;
use tonic::metadata::Ascii;
use tonic::service::Interceptor;
use tonic::transport::Channel;
use tracing::{debug, trace, warn};

use crate::catalogue_manager::{CatalogueEntry, register_plugin_entries};
use crate::child_process::ChildPluginProcess;
use crate::content::{ContentMismatch, InteractionContents};
use crate::mock_server::{MockServerConfig, MockServerDetails};
use crate::plugin_models::{CompareContentResult, PactPlugin, PactPluginManifest};
use crate::proto::{body, Body, Catalogue, CompareContentsRequest, CompareContentsResponse, ConfigureInteractionRequest, ConfigureInteractionResponse, GenerateContentRequest, GenerateContentResponse, InitPluginRequest, InitPluginResponse, InteractionData, metadata_value, MetadataValue, MockServerRequest, MockServerResults, PluginConfiguration, ShutdownMockServerRequest, ShutdownMockServerResponse, start_mock_server_response, StartMockServerRequest, StartMockServerResponse, verification_preparation_response, VerificationPreparationRequest, VerificationPreparationResponse, verify_interaction_response, VerifyInteractionRequest, VerifyInteractionResponse};
use crate::proto::interaction_response::MarkupType;
use crate::proto::pact_plugin_client::PactPluginClient;
use crate::utils::{optional_string, proto_struct_to_json, proto_struct_to_map, proto_value_to_json, to_proto_struct, to_proto_value};
use crate::verification::{InteractionVerificationData, InteractionVerificationResult};

/// Trait with remote-calling methods for a running gRPC-based plugin
#[async_trait]
pub trait PactPluginRpc {
  /// Send an init request to the plugin process
  async fn init_plugin(&mut self, request: InitPluginRequest) -> anyhow::Result<InitPluginResponse>;

  /// Send a compare contents request to the plugin process
  async fn compare_contents(&self, request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse>;

  /// Send a configure contents request to the plugin process
  async fn configure_interaction(&self, request: ConfigureInteractionRequest) -> anyhow::Result<ConfigureInteractionResponse>;

  /// Send a generate content request to the plugin
  async fn generate_content(&self, request: GenerateContentRequest) -> anyhow::Result<GenerateContentResponse>;

  /// Start a mock server
  async fn start_mock_server(&self, request: StartMockServerRequest) -> anyhow::Result<StartMockServerResponse>;

  /// Shutdown a running mock server
  async fn shutdown_mock_server(&self, request: ShutdownMockServerRequest) -> anyhow::Result<ShutdownMockServerResponse>;

  /// Get the matching results from a running mock server
  async fn get_mock_server_results(&self, request: MockServerRequest) -> anyhow::Result<MockServerResults>;

  /// Prepare an interaction for verification. This should return any data required to construct any request
  /// so that it can be amended before the verification is run.
  async fn prepare_interaction_for_verification(&self, request: VerificationPreparationRequest) -> anyhow::Result<VerificationPreparationResponse>;

  /// Execute the verification for the interaction.
  async fn verify_interaction(&self, request: VerifyInteractionRequest) -> anyhow::Result<VerifyInteractionResponse>;

  /// Updates the catalogue. This will be sent when the core catalogue has been updated (probably by a plugin loading).
  async fn update_catalogue(&self, request: Catalogue) -> anyhow::Result<()>;
}

/// Running plugin details of a gRPC-based plugin
#[derive(Debug, Clone)]
pub struct GrpcPactPlugin {
  /// Manifest for this plugin
  pub manifest: PactPluginManifest,

  /// Running child process
  pub child: Arc<ChildPluginProcess>,

  /// Count of access to the plugin. If this is ever zero, the plugin process will be shutdown
  access_count: Arc<AtomicUsize>
}

#[async_trait]
impl PactPluginRpc for GrpcPactPlugin {
  /// Send an init request to the plugin process
  async fn init_plugin(&mut self, request: InitPluginRequest) -> anyhow::Result<InitPluginResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.init_plugin(Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  /// Send a compare contents request to the plugin process
  async fn compare_contents(&self, request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.compare_contents(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  /// Send a configure contents request to the plugin process
  async fn configure_interaction(&self, request: ConfigureInteractionRequest) -> anyhow::Result<ConfigureInteractionResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.configure_interaction(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  /// Send a generate content request to the plugin
  async fn generate_content(&self, request: GenerateContentRequest) -> anyhow::Result<GenerateContentResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.generate_content(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn start_mock_server(&self, request: StartMockServerRequest) -> anyhow::Result<StartMockServerResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.start_mock_server(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn shutdown_mock_server(&self, request: ShutdownMockServerRequest) -> anyhow::Result<ShutdownMockServerResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.shutdown_mock_server(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn get_mock_server_results(&self, request: MockServerRequest) -> anyhow::Result<MockServerResults> {
    let mut client = self.get_plugin_client().await?;
    let response = client.get_mock_server_results(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn prepare_interaction_for_verification(&self, request: VerificationPreparationRequest) -> anyhow::Result<VerificationPreparationResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.prepare_interaction_for_verification(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn verify_interaction(&self, request: VerifyInteractionRequest) -> anyhow::Result<VerifyInteractionResponse> {
    let mut client = self.get_plugin_client().await?;
    let response = client.verify_interaction(tonic::Request::new(request)).await?;
    Ok(response.get_ref().clone())
  }

  async fn update_catalogue(&self, request: Catalogue) -> anyhow::Result<()> {
    let mut client = self.get_plugin_client().await?;
    client.update_catalogue(tonic::Request::new(request)).await?;
    Ok(())
  }
}

impl GrpcPactPlugin {
  /// Create a new Plugin
  pub fn new(manifest: &PactPluginManifest, child: ChildPluginProcess) -> Self {
    GrpcPactPlugin {
      manifest: manifest.clone(),
      child: Arc::new(child),
      access_count: Arc::new(AtomicUsize::new(1))
    }
  }

  /// Port the plugin is running on
  pub fn port(&self) -> u16 {
    self.child.port()
  }

  async fn connect_channel(&self) -> anyhow::Result<Channel> {
    let port = self.child.port();
    match Channel::from_shared(format!("http://[::1]:{}", port))?.connect().await {
      Ok(channel) => Ok(channel),
      Err(err) => {
        debug!("IP6 connection failed, will try IP4 address - {err}");
        Channel::from_shared(format!("http://127.0.0.1:{}", port))?.connect().await
          .map_err(|err| anyhow!(err))
      }
    }
  }

  async fn get_plugin_client(&self) -> anyhow::Result<PactPluginClient<InterceptedService<Channel, PactPluginInterceptor>>> {
    let channel = self.connect_channel().await?;
    let interceptor = PactPluginInterceptor::new(self.child.plugin_info.server_key.as_str())?;
    Ok(PactPluginClient::with_interceptor(channel, interceptor))
  }

  fn setup_matching_rules(rules_map: &HashMap<String, crate::proto::MatchingRules>) -> anyhow::Result<Option<MatchingRuleCategory>> {
    if !rules_map.is_empty() {
      let mut rules = hashmap!{};
      for (k, rule_list) in rules_map {
        let mut vec = vec![];
        for rule in &rule_list.rule {
          let mr = MatchingRule::create(rule.r#type.as_str(), &rule.values.as_ref().map(|rule| {
            proto_struct_to_json(rule)
          }).unwrap_or_default())?;
          vec.push(mr);
        }
        rules.insert(DocPath::new(k)?, RuleList {
          rules: vec,
          rule_logic: RuleLogic::And,
          cascaded: false
        });
      }
      Ok(Some(MatchingRuleCategory { name: Category::BODY, rules }))
    } else {
      Ok(None)
    }
  }
}

#[async_trait]
impl PactPlugin for GrpcPactPlugin {
  fn manifest(&self) -> PactPluginManifest {
    self.manifest.clone()
  }

  fn kill(&self) {
    self.child.kill();
  }

  fn update_access(&mut self) {
    let count = self.access_count.fetch_add(1, Ordering::SeqCst);
    trace!("update_access: Plugin {}/{} access is now {}", self.manifest.name,
      self.manifest.version, count + 1);
  }

  fn drop_access(&mut self) -> usize {
    let check = self.access_count.fetch_update(Ordering::SeqCst,
                                               Ordering::SeqCst, |count| {
        if count > 0 {
          Some(count - 1)
        } else {
          None
        }
      });
    let count = if let Ok(v) = check {
      if v > 0 { v - 1 } else { v }
    } else {
      0
    };
    trace!("drop_access: Plugin {}/{} access is now {}", self.manifest.name, self.manifest.version,
      count);
    count
  }

  fn boxed(&self) -> Box<dyn PactPlugin + Send + Sync> {
    Box::new(self.clone())
  }

  fn arced(&self) -> Arc<dyn PactPlugin + Send + Sync> {
    Arc::new(self.clone())
  }

  async fn publish_updated_catalogue(&self, catalogue: &[CatalogueEntry]) -> anyhow::Result<()> {
    let request = Catalogue {
      catalogue: catalogue.iter()
        .map(|entry| crate::proto::CatalogueEntry {
          r#type: entry.entry_type.to_proto_type() as i32,
          key: entry.key.clone(),
          values: entry.values.clone()
        }).collect()
    };
    self.update_catalogue(request).await
  }

  async fn generate_contents(
    &self,
    request: crate::plugin_models::GenerateContentRequest
  ) -> anyhow::Result<OptionalBody> {
    let request = crate::proto::GenerateContentRequest {
      contents: Some(crate::proto::Body {
        content_type: request.content_type.to_string(),
        content: Some(request.content.value().unwrap_or_default().to_vec()),
        content_type_hint: body::ContentTypeHint::Default as i32
      }),
      generators: request.generators.iter().map(|(k, v)| {
        (k.clone(), crate::proto::Generator {
          r#type: v.name(),
          values: Some(to_proto_struct(&v.values().iter()
            .map(|(k, v)| (k.to_string(), v.clone())).collect())),
        })
      }).collect(),
      plugin_configuration: Some(crate::proto::PluginConfiguration {
        pact_configuration: request.plugin_data.as_ref().map(to_proto_struct),
        interaction_configuration: request.interaction_data.as_ref().map(to_proto_struct),
        .. crate::proto::PluginConfiguration::default()
      }),
      test_context: Some(to_proto_struct(&request.test_context.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())),
      .. crate::proto::GenerateContentRequest::default()
    };
    self.generate_content(request).await.map(|response| {
      match response.contents {
        Some(contents) => {
          OptionalBody::Present(
            Bytes::from(contents.content.unwrap_or_default()),
            ContentType::parse(contents.content_type.as_str()).ok(),
            None
          )
        }
        None => OptionalBody::Empty
      }
    })
  }

  async fn match_contents(
    &self,
    request: crate::plugin_models::CompareContentRequest
  ) -> anyhow::Result<CompareContentResult> {
    let request = crate::proto::CompareContentsRequest {
      expected: Some(Body {
        content_type: request.expected_contents.content_type().unwrap_or_default().to_string(),
        content: request.expected_contents.value().map(|b| b.to_vec()),
        content_type_hint: body::ContentTypeHint::Default as i32
      }),
      actual: Some(Body {
        content_type: request.actual_contents.content_type().unwrap_or_default().to_string(),
        content: request.actual_contents.value().map(|b| b.to_vec()),
        content_type_hint: body::ContentTypeHint::Default as i32
      }),
      allow_unexpected_keys: request.allow_unexpected_keys,
      rules: request.matching_rules.iter().map(|(k, r)| {
        (k.to_string(), crate::proto::MatchingRules {
          rule: r.rules.iter().map(|rule|{
            crate::proto::MatchingRule {
              r#type: rule.name(),
              values: Some(to_proto_struct(&rule.values().iter().map(|(k, v)| (k.to_string(), v.clone())).collect())),
            }
          }).collect()
        })
      }).collect(),
      plugin_configuration: request.plugin_configuration.map(|config| PluginConfiguration {
        interaction_configuration: Some(to_proto_struct(&config.interaction_configuration)),
        pact_configuration: Some(to_proto_struct(&config.pact_configuration))
      })
    };
    self.compare_contents(request).await.map(|result| {
      if let Some(mismatch) = result.type_mismatch {
        CompareContentResult::TypeMismatch(mismatch.expected, mismatch.actual)
      } else if !result.error.is_empty() {
        CompareContentResult::Error(result.error.clone())
      } else if !result.results.is_empty() {
        CompareContentResult::Mismatches(
          result.results.iter().map(|(k, v)| {
            (k.clone(), v.mismatches.iter().map(|mismatch| {
              ContentMismatch {
                expected: mismatch.expected.as_ref()
                  .map(|e| from_utf8(&e).unwrap_or_default().to_string())
                  .unwrap_or_default(),
                actual: mismatch.actual.as_ref()
                  .map(|a| from_utf8(&a).unwrap_or_default().to_string())
                  .unwrap_or_default(),
                mismatch: mismatch.mismatch.clone(),
                path: mismatch.path.clone(),
                diff: if mismatch.diff.is_empty() {
                  None
                } else {
                  Some(mismatch.diff.clone())
                },
                mismatch_type: Some(mismatch.mismatch_type.clone())
              }
            }).collect())
          }).collect()
        )
      } else {
        CompareContentResult::OK
      }
    })
  }

  async fn configure_interaction(
    &self,
    content_type: &ContentType,
    definition: &HashMap<String, Value>
  ) -> anyhow::Result<(Vec<InteractionContents>, Option<crate::content::PluginConfiguration>)> {
    let request = ConfigureInteractionRequest {
      content_type: content_type.to_string(),
      contents_config: Some(to_proto_struct(&definition)),
    };
    match PactPluginRpc::configure_interaction(self, request).await {
      Ok(response) => {
        debug!("Got response: {:?}", response);
        if response.error.is_empty() {
          let mut results = vec![];

          for response in &response.interaction {
            let body = match &response.contents {
              Some(body) => {
                let returned_content_type = ContentType::parse(body.content_type.as_str()).ok();
                let contents = body.content.as_ref().cloned().unwrap_or_default();
                OptionalBody::Present(Bytes::from(contents), returned_content_type,
                                      Some(match body.content_type_hint() {
                                        body::ContentTypeHint::Text => ContentTypeHint::TEXT,
                                        body::ContentTypeHint::Binary => ContentTypeHint::BINARY,
                                        body::ContentTypeHint::Default => ContentTypeHint::DEFAULT,
                                      }))
              },
              None => OptionalBody::Missing
            };

            let rules = Self::setup_matching_rules(&response.rules)?;

            let generators = if !response.generators.is_empty() || !response.metadata_generators.is_empty() {
              let mut categories = hashmap!{};

              if !response.generators.is_empty() {
                let mut generators = hashmap!{};
                for (k, gen) in &response.generators {
                  generators.insert(DocPath::new(k)?,
                                    Generator::create(gen.r#type.as_str(),
                                                      &gen.values.as_ref().map(|attr| proto_struct_to_json(attr)).unwrap_or_default())?);
                }
                categories.insert(GeneratorCategory::BODY, generators);
              }

              if !response.metadata_generators.is_empty() {
                let mut generators = hashmap!{};
                for (k, gen) in &response.metadata_generators {
                  generators.insert(DocPath::new(k)?,
                                    Generator::create(gen.r#type.as_str(),
                                                      &gen.values.as_ref().map(|attr| proto_struct_to_json(attr)).unwrap_or_default())?);
                }
                categories.insert(GeneratorCategory::METADATA, generators);
              }

              Some(Generators { categories })
            } else {
              None
            };

            let metadata = response.message_metadata.as_ref().map(|md| proto_struct_to_map(md));
            let metadata_rules = Self::setup_matching_rules(&response.metadata_rules)?;

            let plugin_config = if let Some(plugin_configuration) = &response.plugin_configuration {
              crate::content::PluginConfiguration {
                interaction_configuration: plugin_configuration.interaction_configuration.as_ref()
                  .map(|val| proto_struct_to_map(val)).unwrap_or_default(),
                pact_configuration: plugin_configuration.pact_configuration.as_ref()
                  .map(|val| proto_struct_to_map(val)).unwrap_or_default()
              }
            } else {
              crate::content::PluginConfiguration::default()
            };

            debug!("body={}", body);
            debug!("rules={:?}", rules);
            debug!("generators={:?}", generators);
            debug!("metadata={:?}", metadata);
            debug!("metadata_rules={:?}", metadata_rules);
            debug!("pluginConfig={:?}", plugin_config);

            results.push(InteractionContents {
              part_name: response.part_name.clone(),
              body,
              rules,
              generators,
              metadata,
              metadata_rules,
              plugin_config,
              interaction_markup: response.interaction_markup.clone(),
              interaction_markup_type: match response.interaction_markup_type() {
                MarkupType::Html => "HTML".to_string(),
                _ => "COMMON_MARK".to_string(),
              }
            })
          }

          Ok((results, response.plugin_configuration.map(|config| crate::content::PluginConfiguration::from(config))))
        } else {
          Err(anyhow!("Request to configure interaction failed: {}", response.error))
        }
      }
      Err(err) => Err(err)
    }
  }

  async fn verify_interaction(
    &self,
    pact: &V4Pact,
    interaction: &(dyn V4Interaction + Send + Sync),
    verification_data: &InteractionVerificationData,
    config: &HashMap<String, Value>
  ) -> anyhow::Result<InteractionVerificationResult> {
    let request = VerifyInteractionRequest {
      pact: pact.to_json(PactSpecification::V4)?.to_string(),
      interaction_key: interaction.unique_key(),
      config: Some(to_proto_struct(config)),
      interaction_data: Some(InteractionData {
        body: Some((&verification_data.request_data).into()),
        metadata: verification_data.metadata.iter().map(|(k, v)| {
          (k.clone(), MetadataValue { value: Some(match v {
            Either::Left(value) => metadata_value::Value::NonBinaryValue(to_proto_value(value)),
            Either::Right(b) => metadata_value::Value::BinaryValue(b.to_vec())
          }) })
        }).collect()
      })
    };

    let response = PactPluginRpc::verify_interaction(self, request).await?;
    let validation_response = response.response
      .ok_or_else(|| anyhow!("Did not get a valid response from the verification call"))?;
    match &validation_response {
      verify_interaction_response::Response::Error(err) => Err(anyhow!("Failed to verify the request: {}", err)),
      verify_interaction_response::Response::Result(data) => Ok(data.into())
    }
  }

  async fn prepare_interaction_for_verification(
    &self,
    pact: &V4Pact,
    interaction: &(dyn V4Interaction + Send + Sync),
    context: &HashMap<String, Value>
  ) -> anyhow::Result<InteractionVerificationData> {
    let request = VerificationPreparationRequest {
      pact: pact.to_json(PactSpecification::V4)?.to_string(),
      interaction_key: interaction.unique_key(),
      config: Some(to_proto_struct(context))
    };

    let response = PactPluginRpc::prepare_interaction_for_verification(self, request).await?;
    let validation_response = response.response
      .ok_or_else(|| anyhow!("Did not get a valid response from the prepare interaction for verification call"))?;
    match &validation_response {
      verification_preparation_response::Response::Error(err) => Err(anyhow!("Failed to prepare the request: {}", err)),
      verification_preparation_response::Response::InteractionData(data) => {
        let content_type = data.body.as_ref().and_then(|body| ContentType::parse(body.content_type.as_str()).ok());
        Ok(InteractionVerificationData {
          request_data: data.body.as_ref()
            .and_then(|body| body.content.as_ref())
            .map(|body| OptionalBody::Present(Bytes::from(body.clone()), content_type, None)).unwrap_or_default(),
          metadata: data.metadata.iter().map(|(k, v)| {
            let value = match &v.value {
              Some(v) => match &v {
                metadata_value::Value::NonBinaryValue(v) => Either::Left(proto_value_to_json(v)),
                metadata_value::Value::BinaryValue(b) => Either::Right(Bytes::from(b.clone()))
              }
              None => Either::Left(Value::Null)
            };
            (k.clone(), value)
          }).collect()
        })
      }
    }
  }

  async fn start_mock_server(
    &self,
    config: &MockServerConfig,
    pact: Box<dyn Pact + Send + Sync>,
    test_context: HashMap<String, Value>
  ) -> anyhow::Result<MockServerDetails> {
    let request = StartMockServerRequest {
      host_interface: config.host_interface.clone().unwrap_or_default(),
      port: config.port,
      tls: config.tls,
      pact: pact.to_json(PactSpecification::V4)?.to_string(),
      test_context: Some(to_proto_struct(&test_context))
    };
    let response = PactPluginRpc::start_mock_server(self, request).await?;
    let mock_server_response = response.response
      .ok_or_else(|| anyhow!("Did not get a valid response from the start mock server call"))?;
    match mock_server_response {
      start_mock_server_response::Response::Error(err) => Err(anyhow!("Mock server failed to start: {}", err)),
      start_mock_server_response::Response::Details(details) => Ok(MockServerDetails {
        key: details.key.clone(),
        base_url: details.address.clone(),
        port: details.port,
        plugin: self.boxed()
      })
    }
  }

  async fn get_mock_server_results(
    &self,
    mock_server_key: &str
  ) -> anyhow::Result<Vec<crate::mock_server::MockServerResults>> {
    let request = MockServerRequest {
      server_key: mock_server_key.to_string()
    };
    let response = PactPluginRpc::get_mock_server_results(self, request).await?;
    if response.ok {
      Ok(vec![])
    } else {
      Ok(response.results.iter().map(|result| {
        crate::mock_server::MockServerResults {
          path: result.path.clone(),
          error: result.error.clone(),
          mismatches: result.mismatches.iter().map(|mismatch| {
            ContentMismatch {
              expected: mismatch.expected.as_ref()
                .map(|e| from_utf8(e.as_slice()).unwrap_or_default().to_string())
                .unwrap_or_default(),
              actual: mismatch.actual.as_ref()
                .map(|a| from_utf8(a.as_slice()).unwrap_or_default().to_string())
                .unwrap_or_default(),
              mismatch: mismatch.mismatch.clone(),
              path: mismatch.path.clone(),
              diff: optional_string(&mismatch.diff),
              mismatch_type: optional_string(&mismatch.mismatch_type)
            }
          }).collect()
        }
      }).collect())
    }
  }

  async fn shutdown_mock_server(
    &self,
    mock_server_key: &str
  ) -> anyhow::Result<Vec<crate::mock_server::MockServerResults>> {
    let request = ShutdownMockServerRequest {
      server_key: mock_server_key.to_string()
    };
    let response = PactPluginRpc::shutdown_mock_server(self, request).await?;
    if response.ok {
      Ok(vec![])
    } else {
      Ok(response.results.iter().map(|result| {
        crate::mock_server::MockServerResults {
          path: result.path.clone(),
          error: result.error.clone(),
          mismatches: result.mismatches.iter().map(|mismatch| {
            ContentMismatch {
              expected: mismatch.expected.as_ref()
                .map(|e| from_utf8(&e).unwrap_or_default().to_string())
                .unwrap_or_default(),
              actual: mismatch.actual.as_ref()
                .map(|a| from_utf8(&a).unwrap_or_default().to_string())
                .unwrap_or_default(),
              mismatch: mismatch.mismatch.clone(),
              path: mismatch.path.clone(),
              diff: optional_string(&mismatch.diff),
              mismatch_type: optional_string(&mismatch.mismatch_type)
            }
          }).collect()
        }
      }).collect())
    }
  }
}

/// Interceptor to inject the server key as an authorisation header
#[derive(Clone, Debug)]
struct PactPluginInterceptor {
  /// Server key to inject
  server_key: tonic::metadata::MetadataValue<Ascii>
}

impl PactPluginInterceptor {
  fn new(server_key: &str) -> anyhow::Result<Self> {
    let token = tonic::metadata::MetadataValue::try_from(server_key)?;
    Ok(PactPluginInterceptor {
      server_key: token
    })
  }
}

impl Interceptor for PactPluginInterceptor {
  fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
    request.metadata_mut().insert("authorization", self.server_key.clone());
    Ok(request)
  }
}

/// Internal function: public for testing
pub async fn init_handshake(manifest: &PactPluginManifest, plugin: &mut (dyn PactPluginRpc + Send + Sync)) -> anyhow::Result<()> {
  let request = InitPluginRequest {
    implementation: "plugin-driver-rust".to_string(),
    version: option_env!("CARGO_PKG_VERSION").unwrap_or("0").to_string()
  };
  let response = plugin.init_plugin(request).await?;
  debug!("Got init response {:?} from plugin {}", response, manifest.name);
  register_plugin_entries(manifest, &response.catalogue);
  Ok(())
}

#[cfg(not(windows))]
pub(crate) async fn start_plugin_process(manifest: &PactPluginManifest) -> anyhow::Result<GrpcPactPlugin> {
  debug!("Starting plugin with manifest {:?}", manifest);

  let os_info = os_info::get();
  debug!("Detected OS: {}", os_info);
  let mut path = if let Some(entry_point) = manifest.entry_points.get(&os_info.to_string()) {
    PathBuf::from(entry_point)
  } else if os_info.os_type() == Type::Windows && manifest.entry_points.contains_key("windows") {
    PathBuf::from(manifest.entry_points.get("windows").unwrap())
  } else {
    PathBuf::from(&manifest.entry_point)
  };

  if !path.is_absolute() || !path.exists() {
    path = PathBuf::from(manifest.plugin_dir.clone()).join(path);
  }
  debug!("Starting plugin using {:?}", &path);

  let log_level = max_level();
  let mut child_command = Command::new(path.clone());
  let mut child_command = child_command
    .env("LOG_LEVEL", log_level.to_string())
    .env("RUST_LOG", log_level.to_string())
    .current_dir(manifest.plugin_dir.clone());

  if let Some(args) = &manifest.args {
    child_command = child_command.args(args);
  }

  let child = child_command
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| anyhow!("Was not able to start plugin process for '{}' - {}",
      path.to_string_lossy(), err))?;
  let child_pid = child.id().unwrap_or_default();
  debug!("Plugin {} started with PID {}", manifest.name, child_pid);

  match ChildPluginProcess::new(child, manifest).await {
    Ok(child) => Ok(GrpcPactPlugin::new(manifest, child)),
    Err(err) => {
      let mut s = System::new();
      s.refresh_processes();
      if let Some(process) = s.process(Pid::from_u32(child_pid)) {
        process.kill_with(Signal::Term);
      } else {
        warn!("Child process with PID {} was not found", child_pid);
      }
      Err(err)
    }
  }
}

#[cfg(windows)]
async fn start_plugin_process(manifest: &PactPluginManifest) -> anyhow::Result<GrpcPactPlugin> {
  debug!("Starting plugin with manifest {:?}", manifest);

  let mut path = if let Some(entry) = manifest.entry_points.get("windows") {
    PathBuf::from(entry)
  } else {
    PathBuf::from(&manifest.entry_point)
  };
  if !path.is_absolute() || !path.exists() {
    path = PathBuf::from(manifest.plugin_dir.clone()).join(path);
  }
  debug!("Starting plugin using {:?}", &path);

  let log_level = max_level();
  let mut child_command = Command::new(path.clone());
  let mut child_command = child_command
    .env("LOG_LEVEL", log_level.to_string())
    .env("RUST_LOG", log_level.to_string())
    .current_dir(manifest.plugin_dir.clone());

  if let Some(args) = &manifest.args {
    child_command = child_command.args(args);
  }

  let child = child_command
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| anyhow!("Was not able to start plugin process for '{}' - {}",
      path.to_string_lossy(), err))?;
  let child_pid = child.id();
  debug!("Plugin {} started with PID {}", manifest.name, child_pid);

  match ChildPluginProcess::new(child, manifest).await {
    Ok(child) => Ok(GrpcPactPlugin::new(manifest, child)),
    Err(err) => {
      let mut s = System::new();
      s.refresh_processes();
      if let Some(process) = s.process(Pid::from_u32(child_pid)) {
        process.kill_with(Signal::Term);
      } else {
        warn!("Child process with PID {} was not found", child_pid);
      }
      Err(err)
    }
  }
}

#[cfg(test)]
pub(crate) mod tests {

}
