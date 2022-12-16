use pact_models::prelude::OptionalBody;
use pact_models::content_types::ContentTypeHint;

// Build with PACT_PLUGIN_BUILD_PROTOBUFS set, then include the following
// tonic::include_proto!("io.pact.plugin");

// ------------------ Generated --------------------------//
/// Request to verify the plugin has loaded OK
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InitPluginRequest {
  /// Implementation calling the plugin
  #[prost(string, tag = "1")]
  pub implementation: ::prost::alloc::string::String,
  /// Version of the implementation
  #[prost(string, tag = "2")]
  pub version: ::prost::alloc::string::String,
}
/// Entry to be added to the core catalogue. Each entry describes one of the features the plugin provides.
/// Entries will be stored in the catalogue under the key "plugin/$name/$type/$key".
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CatalogueEntry {
  /// Entry type
  #[prost(enumeration = "catalogue_entry::EntryType", tag = "1")]
  pub r#type: i32,
  /// Entry key
  #[prost(string, tag = "2")]
  pub key: ::prost::alloc::string::String,
  /// Associated data required for the entry. For CONTENT_MATCHER and CONTENT_GENERATOR types, a "content-types"
  /// value (separated by semi-colons) is required for all the content types the plugin supports.
  #[prost(map = "string, string", tag = "3")]
  pub values: ::std::collections::HashMap<
    ::prost::alloc::string::String,
    ::prost::alloc::string::String,
  >,
}
/// Nested message and enum types in `CatalogueEntry`.
pub mod catalogue_entry {
  #[derive(
  Clone,
  Copy,
  Debug,
  PartialEq,
  Eq,
  Hash,
  PartialOrd,
  Ord,
  ::prost::Enumeration
  )]
  #[repr(i32)]
  pub enum EntryType {
    /// Matcher for contents of messages, requests or response bodies
    ContentMatcher = 0,
    /// Generator for contents of messages, requests or response bodies
    ContentGenerator = 1,
    /// Transport for a network protocol
    Transport = 2,
    /// Matching rule for content field/values
    Matcher = 3,
    /// Type of interaction
    Interaction = 4,
  }
  impl EntryType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
      match self {
        EntryType::ContentMatcher => "CONTENT_MATCHER",
        EntryType::ContentGenerator => "CONTENT_GENERATOR",
        EntryType::Transport => "TRANSPORT",
        EntryType::Matcher => "MATCHER",
        EntryType::Interaction => "INTERACTION",
      }
    }
  }
}
/// Response to init plugin, providing the catalogue entries the plugin provides
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InitPluginResponse {
  /// List of entries the plugin supports
  #[prost(message, repeated, tag = "1")]
  pub catalogue: ::prost::alloc::vec::Vec<CatalogueEntry>,
}
/// Catalogue of Core Pact + Plugin features
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Catalogue {
  /// List of entries from the core catalogue
  #[prost(message, repeated, tag = "1")]
  pub catalogue: ::prost::alloc::vec::Vec<CatalogueEntry>,
}
/// Message representing a request, response or message body
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Body {
  /// The content type of the body in MIME format (i.e. application/json)
  #[prost(string, tag = "1")]
  pub content_type: ::prost::alloc::string::String,
  /// Bytes of the actual content
  #[prost(message, optional, tag = "2")]
  pub content: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
  /// Content type override to apply (if required). If omitted, the default rules of the Pact implementation
  /// will be used
  #[prost(enumeration = "body::ContentTypeHint", tag = "3")]
  pub content_type_hint: i32,
}
/// Nested message and enum types in `Body`.
pub mod body {
  /// Enum of content type override. This is a hint on how the content type should be treated.
  #[derive(
  Clone,
  Copy,
  Debug,
  PartialEq,
  Eq,
  Hash,
  PartialOrd,
  Ord,
  ::prost::Enumeration
  )]
  #[repr(i32)]
  pub enum ContentTypeHint {
    /// Determine the form of the content using the default rules of the Pact implementation
    Default = 0,
    /// Contents must always be treated as a text form
    Text = 1,
    /// Contents must always be treated as a binary form
    Binary = 2,
  }
  impl ContentTypeHint {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
      match self {
        ContentTypeHint::Default => "DEFAULT",
        ContentTypeHint::Text => "TEXT",
        ContentTypeHint::Binary => "BINARY",
      }
    }
  }
}
/// Request to preform a comparison on an actual body given the expected one
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompareContentsRequest {
  /// Expected body from the Pact interaction
  #[prost(message, optional, tag = "1")]
  pub expected: ::core::option::Option<Body>,
  /// Actual received body
  #[prost(message, optional, tag = "2")]
  pub actual: ::core::option::Option<Body>,
  /// If unexpected keys or attributes should be allowed. Setting this to false results in additional keys or fields
  /// will cause a mismatch
  #[prost(bool, tag = "3")]
  pub allow_unexpected_keys: bool,
  /// Map of expressions to matching rules. The expressions follow the documented Pact matching rule expressions
  #[prost(map = "string, message", tag = "4")]
  pub rules: ::std::collections::HashMap<
    ::prost::alloc::string::String,
    MatchingRules,
  >,
  /// Additional data added to the Pact/Interaction by the plugin
  #[prost(message, optional, tag = "5")]
  pub plugin_configuration: ::core::option::Option<PluginConfiguration>,
}
/// Indicates that there was a mismatch with the content type
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ContentTypeMismatch {
  /// Expected content type (MIME format)
  #[prost(string, tag = "1")]
  pub expected: ::prost::alloc::string::String,
  /// Actual content type received (MIME format)
  #[prost(string, tag = "2")]
  pub actual: ::prost::alloc::string::String,
}
/// A mismatch for an particular item of content
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ContentMismatch {
  /// Expected data bytes
  #[prost(message, optional, tag = "1")]
  pub expected: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
  /// Actual data bytes
  #[prost(message, optional, tag = "2")]
  pub actual: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
  /// Description of the mismatch
  #[prost(string, tag = "3")]
  pub mismatch: ::prost::alloc::string::String,
  /// Path to the item that was matched. This is the value as per the documented Pact matching rule expressions.
  #[prost(string, tag = "4")]
  pub path: ::prost::alloc::string::String,
  /// Optional diff of the contents
  #[prost(string, tag = "5")]
  pub diff: ::prost::alloc::string::String,
}
/// List of content mismatches
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ContentMismatches {
  #[prost(message, repeated, tag = "1")]
  pub mismatches: ::prost::alloc::vec::Vec<ContentMismatch>,
}
/// Response to the CompareContentsRequest with the results of the comparison
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompareContentsResponse {
  /// Error message if an error occurred. If this field is set, the remaining fields will be ignored and the
  /// verification marked as failed
  #[prost(string, tag = "1")]
  pub error: ::prost::alloc::string::String,
  /// There was a mismatch with the types of content. If this is set, the results may not be set.
  #[prost(message, optional, tag = "2")]
  pub type_mismatch: ::core::option::Option<ContentTypeMismatch>,
  /// Results of the match, keyed by matching rule expression
  #[prost(map = "string, message", tag = "3")]
  pub results: ::std::collections::HashMap<
    ::prost::alloc::string::String,
    ContentMismatches,
  >,
}
/// Request to configure/setup an interaction so that it can be verified later
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfigureInteractionRequest {
  /// Content type of the interaction (MIME format)
  #[prost(string, tag = "1")]
  pub content_type: ::prost::alloc::string::String,
  /// This is data specified by the user in the consumer test
  #[prost(message, optional, tag = "2")]
  pub contents_config: ::core::option::Option<::prost_types::Struct>,
}
/// Represents a matching rule
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MatchingRule {
  /// Type of the matching rule
  #[prost(string, tag = "1")]
  pub r#type: ::prost::alloc::string::String,
  /// Associated data for the matching rule
  #[prost(message, optional, tag = "2")]
  pub values: ::core::option::Option<::prost_types::Struct>,
}
/// List of matching rules
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MatchingRules {
  #[prost(message, repeated, tag = "1")]
  pub rule: ::prost::alloc::vec::Vec<MatchingRule>,
}
/// Example generator
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Generator {
  /// Type of generator
  #[prost(string, tag = "1")]
  pub r#type: ::prost::alloc::string::String,
  /// Associated data for the generator
  #[prost(message, optional, tag = "2")]
  pub values: ::core::option::Option<::prost_types::Struct>,
}
/// Plugin configuration added to the pact file by the ConfigureInteraction step
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PluginConfiguration {
  /// Data to be persisted against the interaction
  #[prost(message, optional, tag = "1")]
  pub interaction_configuration: ::core::option::Option<::prost_types::Struct>,
  /// Data to be persisted in the Pact file metadata (Global data)
  #[prost(message, optional, tag = "2")]
  pub pact_configuration: ::core::option::Option<::prost_types::Struct>,
}
/// Response to the configure/setup an interaction request
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InteractionResponse {
  /// Contents for the interaction
  #[prost(message, optional, tag = "1")]
  pub contents: ::core::option::Option<Body>,
  /// All matching rules to apply
  #[prost(map = "string, message", tag = "2")]
  pub rules: ::std::collections::HashMap<
    ::prost::alloc::string::String,
    MatchingRules,
  >,
  /// Generators to apply
  #[prost(map = "string, message", tag = "3")]
  pub generators: ::std::collections::HashMap<
    ::prost::alloc::string::String,
    Generator,
  >,
  /// For message interactions, any metadata to be applied
  #[prost(message, optional, tag = "4")]
  pub message_metadata: ::core::option::Option<::prost_types::Struct>,
  /// Plugin specific data to be persisted in the pact file
  #[prost(message, optional, tag = "5")]
  pub plugin_configuration: ::core::option::Option<PluginConfiguration>,
  /// Markdown/HTML formatted text representation of the interaction
  #[prost(string, tag = "6")]
  pub interaction_markup: ::prost::alloc::string::String,
  #[prost(enumeration = "interaction_response::MarkupType", tag = "7")]
  pub interaction_markup_type: i32,
  /// Description of what part this interaction belongs to (in the case of there being more than one, for instance,
  /// request/response messages)
  #[prost(string, tag = "8")]
  pub part_name: ::prost::alloc::string::String,
}
/// Nested message and enum types in `InteractionResponse`.
pub mod interaction_response {
  /// Type of markup used
  #[derive(
  Clone,
  Copy,
  Debug,
  PartialEq,
  Eq,
  Hash,
  PartialOrd,
  Ord,
  ::prost::Enumeration
  )]
  #[repr(i32)]
  pub enum MarkupType {
    /// CommonMark format
    CommonMark = 0,
    /// HTML format
    Html = 1,
  }
  impl MarkupType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
      match self {
        MarkupType::CommonMark => "COMMON_MARK",
        MarkupType::Html => "HTML",
      }
    }
  }
}
/// Response to the configure/setup an interaction request
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfigureInteractionResponse {
  /// If an error occurred. In this case, the other fields will be ignored/not set
  #[prost(string, tag = "1")]
  pub error: ::prost::alloc::string::String,
  /// The actual response if no error occurred.
  #[prost(message, repeated, tag = "2")]
  pub interaction: ::prost::alloc::vec::Vec<InteractionResponse>,
  /// Plugin specific data to be persisted in the pact file
  #[prost(message, optional, tag = "3")]
  pub plugin_configuration: ::core::option::Option<PluginConfiguration>,
}
/// Request to generate the contents using any defined generators
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GenerateContentRequest {
  /// Original contents
  #[prost(message, optional, tag = "1")]
  pub contents: ::core::option::Option<Body>,
  /// Generators to apply
  #[prost(map = "string, message", tag = "2")]
  pub generators: ::std::collections::HashMap<
    ::prost::alloc::string::String,
    Generator,
  >,
  /// Additional data added to the Pact/Interaction by the plugin
  #[prost(message, optional, tag = "3")]
  pub plugin_configuration: ::core::option::Option<PluginConfiguration>,
  /// Context data provided by the test framework
  #[prost(message, optional, tag = "4")]
  pub test_context: ::core::option::Option<::prost_types::Struct>,
  #[prost(enumeration = "generate_content_request::TestMode", tag = "5")]
  pub test_mode: i32,
  #[prost(enumeration = "generate_content_request::ContentFor", tag = "6")]
  pub content_for: i32,
}
/// Nested message and enum types in `GenerateContentRequest`.
pub mod generate_content_request {
  /// The mode of the generation, if running from a consumer test or during provider verification
  #[derive(
  Clone,
  Copy,
  Debug,
  PartialEq,
  Eq,
  Hash,
  PartialOrd,
  Ord,
  ::prost::Enumeration
  )]
  #[repr(i32)]
  pub enum TestMode {
    Unknown = 0,
    /// Running on the consumer side
    Consumer = 1,
    /// Running on the provider side
    Provider = 2,
  }
  impl TestMode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
      match self {
        TestMode::Unknown => "Unknown",
        TestMode::Consumer => "Consumer",
        TestMode::Provider => "Provider",
      }
    }
  }
  /// Which part the content is for
  #[derive(
  Clone,
  Copy,
  Debug,
  PartialEq,
  Eq,
  Hash,
  PartialOrd,
  Ord,
  ::prost::Enumeration
  )]
  #[repr(i32)]
  pub enum ContentFor {
    Request = 0,
    Response = 1,
  }
  impl ContentFor {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
      match self {
        ContentFor::Request => "Request",
        ContentFor::Response => "Response",
      }
    }
  }
}
/// Generated body/message response
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GenerateContentResponse {
  #[prost(message, optional, tag = "1")]
  pub contents: ::core::option::Option<Body>,
}
/// Request to start a mock server
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StartMockServerRequest {
  /// Interface to bind to. Will default to the loopback adapter
  #[prost(string, tag = "1")]
  pub host_interface: ::prost::alloc::string::String,
  /// Port to bind to. Default (or a value of 0) get the OS to open a random port
  #[prost(uint32, tag = "2")]
  pub port: u32,
  /// If TLS should be used (if supported by the mock server)
  #[prost(bool, tag = "3")]
  pub tls: bool,
  /// Pact as JSON to use for the mock server behaviour
  #[prost(string, tag = "4")]
  pub pact: ::prost::alloc::string::String,
  /// Context data provided by the test framework
  #[prost(message, optional, tag = "5")]
  pub test_context: ::core::option::Option<::prost_types::Struct>,
}
/// Response to the start mock server request
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StartMockServerResponse {
  #[prost(oneof = "start_mock_server_response::Response", tags = "1, 2")]
  pub response: ::core::option::Option<start_mock_server_response::Response>,
}
/// Nested message and enum types in `StartMockServerResponse`.
pub mod start_mock_server_response {
  #[allow(clippy::derive_partial_eq_without_eq)]
  #[derive(Clone, PartialEq, ::prost::Oneof)]
  pub enum Response {
    /// If an error occurred
    #[prost(string, tag = "1")]
    Error(::prost::alloc::string::String),
    /// Mock server details
    #[prost(message, tag = "2")]
    Details(super::MockServerDetails),
  }
}
/// Details on a running mock server
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MockServerDetails {
  /// Mock server unique ID
  #[prost(string, tag = "1")]
  pub key: ::prost::alloc::string::String,
  /// Port the mock server is running on
  #[prost(uint32, tag = "2")]
  pub port: u32,
  /// IP address the mock server is bound to. Probably an IP6 address, but may be IP4
  #[prost(string, tag = "3")]
  pub address: ::prost::alloc::string::String,
}
/// Request to shut down a running mock server
/// TODO: replace this with MockServerRequest in the next major version
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShutdownMockServerRequest {
  /// The server ID to shutdown
  #[prost(string, tag = "1")]
  pub server_key: ::prost::alloc::string::String,
}
/// Request for a running mock server by ID
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MockServerRequest {
  /// The server ID to shutdown
  #[prost(string, tag = "1")]
  pub server_key: ::prost::alloc::string::String,
}
/// Result of a request that the mock server received
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MockServerResult {
  /// service + method that was requested
  #[prost(string, tag = "1")]
  pub path: ::prost::alloc::string::String,
  /// If an error occurred trying to handle the request
  #[prost(string, tag = "2")]
  pub error: ::prost::alloc::string::String,
  /// Any mismatches that occurred
  #[prost(message, repeated, tag = "3")]
  pub mismatches: ::prost::alloc::vec::Vec<ContentMismatch>,
}
/// Response to the shut down mock server request
/// TODO: replace this with MockServerResults in the next major version
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShutdownMockServerResponse {
  /// If the mock status is all ok
  #[prost(bool, tag = "1")]
  pub ok: bool,
  /// The results of the test run, will contain an entry for each request received by the mock server
  #[prost(message, repeated, tag = "2")]
  pub results: ::prost::alloc::vec::Vec<MockServerResult>,
}
/// Matching results of the mock server.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MockServerResults {
  /// If the mock status is all ok
  #[prost(bool, tag = "1")]
  pub ok: bool,
  /// The results of the test run, will contain an entry for each request received by the mock server
  #[prost(message, repeated, tag = "2")]
  pub results: ::prost::alloc::vec::Vec<MockServerResult>,
}
/// Request to prepare an interaction for verification
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerificationPreparationRequest {
  /// Pact as JSON to use for the verification
  #[prost(string, tag = "1")]
  pub pact: ::prost::alloc::string::String,
  /// Interaction key for the interaction from the Pact that is being verified
  #[prost(string, tag = "2")]
  pub interaction_key: ::prost::alloc::string::String,
  /// Any data supplied by the user to verify the interaction
  #[prost(message, optional, tag = "3")]
  pub config: ::core::option::Option<::prost_types::Struct>,
}
/// Request metadata value. Will either be a JSON-like value, or binary data
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MetadataValue {
  #[prost(oneof = "metadata_value::Value", tags = "1, 2")]
  pub value: ::core::option::Option<metadata_value::Value>,
}
/// Nested message and enum types in `MetadataValue`.
pub mod metadata_value {
  #[allow(clippy::derive_partial_eq_without_eq)]
  #[derive(Clone, PartialEq, ::prost::Oneof)]
  pub enum Value {
    #[prost(message, tag = "1")]
    NonBinaryValue(::prost_types::Value),
    #[prost(bytes, tag = "2")]
    BinaryValue(::prost::alloc::vec::Vec<u8>),
  }
}
/// Interaction request data to be sent or received for verification
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InteractionData {
  /// Request/Response body as bytes
  #[prost(message, optional, tag = "1")]
  pub body: ::core::option::Option<Body>,
  /// Metadata associated with the request/response
  #[prost(map = "string, message", tag = "2")]
  pub metadata: ::std::collections::HashMap<
    ::prost::alloc::string::String,
    MetadataValue,
  >,
}
/// Response for the prepare an interaction for verification request
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerificationPreparationResponse {
  #[prost(oneof = "verification_preparation_response::Response", tags = "1, 2")]
  pub response: ::core::option::Option<verification_preparation_response::Response>,
}
/// Nested message and enum types in `VerificationPreparationResponse`.
pub mod verification_preparation_response {
  #[allow(clippy::derive_partial_eq_without_eq)]
  #[derive(Clone, PartialEq, ::prost::Oneof)]
  pub enum Response {
    /// If an error occurred
    #[prost(string, tag = "1")]
    Error(::prost::alloc::string::String),
    /// Interaction data required to construct any request
    #[prost(message, tag = "2")]
    InteractionData(super::InteractionData),
  }
}
/// Request data to verify an interaction
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyInteractionRequest {
  /// Interaction data required to construct the request
  #[prost(message, optional, tag = "1")]
  pub interaction_data: ::core::option::Option<InteractionData>,
  /// Any data supplied by the user to verify the interaction
  #[prost(message, optional, tag = "2")]
  pub config: ::core::option::Option<::prost_types::Struct>,
  /// Pact as JSON to use for the verification
  #[prost(string, tag = "3")]
  pub pact: ::prost::alloc::string::String,
  /// Interaction key for the interaction from the Pact that is being verified
  #[prost(string, tag = "4")]
  pub interaction_key: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerificationResultItem {
  #[prost(oneof = "verification_result_item::Result", tags = "1, 2")]
  pub result: ::core::option::Option<verification_result_item::Result>,
}
/// Nested message and enum types in `VerificationResultItem`.
pub mod verification_result_item {
  #[allow(clippy::derive_partial_eq_without_eq)]
  #[derive(Clone, PartialEq, ::prost::Oneof)]
  pub enum Result {
    #[prost(string, tag = "1")]
    Error(::prost::alloc::string::String),
    #[prost(message, tag = "2")]
    Mismatch(super::ContentMismatch),
  }
}
/// Result of running the verification
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerificationResult {
  /// Was the verification successful?
  #[prost(bool, tag = "1")]
  pub success: bool,
  /// Interaction data retrieved from the provider (optional)
  #[prost(message, optional, tag = "2")]
  pub response_data: ::core::option::Option<InteractionData>,
  /// Any mismatches that occurred
  #[prost(message, repeated, tag = "3")]
  pub mismatches: ::prost::alloc::vec::Vec<VerificationResultItem>,
  /// Output for the verification to display to the user
  #[prost(string, repeated, tag = "4")]
  pub output: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// Result of running the verification
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyInteractionResponse {
  #[prost(oneof = "verify_interaction_response::Response", tags = "1, 2")]
  pub response: ::core::option::Option<verify_interaction_response::Response>,
}
/// Nested message and enum types in `VerifyInteractionResponse`.
pub mod verify_interaction_response {
  #[allow(clippy::derive_partial_eq_without_eq)]
  #[derive(Clone, PartialEq, ::prost::Oneof)]
  pub enum Response {
    /// If an error occurred trying to run the verification
    #[prost(string, tag = "1")]
    Error(::prost::alloc::string::String),
    #[prost(message, tag = "2")]
    Result(super::VerificationResult),
  }
}
/// Generated client implementations.
pub mod pact_plugin_client {
  #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
  use tonic::codegen::*;
  use tonic::codegen::http::Uri;
  #[derive(Debug, Clone)]
  pub struct PactPluginClient<T> {
    inner: tonic::client::Grpc<T>,
  }
  impl PactPluginClient<tonic::transport::Channel> {
    /// Attempt to create a new client by connecting to a given endpoint.
    pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
      where
        D: std::convert::TryInto<tonic::transport::Endpoint>,
        D::Error: Into<StdError>,
    {
      let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
      Ok(Self::new(conn))
    }
  }
  impl<T> PactPluginClient<T>
    where
      T: tonic::client::GrpcService<tonic::body::BoxBody>,
      T::Error: Into<StdError>,
      T::ResponseBody: Body<Data = Bytes> + Send + 'static,
      <T::ResponseBody as Body>::Error: Into<StdError> + Send,
  {
    pub fn new(inner: T) -> Self {
      let inner = tonic::client::Grpc::new(inner);
      Self { inner }
    }
    pub fn with_origin(inner: T, origin: Uri) -> Self {
      let inner = tonic::client::Grpc::with_origin(inner, origin);
      Self { inner }
    }
    pub fn with_interceptor<F>(
      inner: T,
      interceptor: F,
    ) -> PactPluginClient<InterceptedService<T, F>>
      where
        F: tonic::service::Interceptor,
        T::ResponseBody: Default,
        T: tonic::codegen::Service<
          http::Request<tonic::body::BoxBody>,
          Response = http::Response<
            <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
          >,
        >,
        <T as tonic::codegen::Service<
          http::Request<tonic::body::BoxBody>,
        >>::Error: Into<StdError> + Send + Sync,
    {
      PactPluginClient::new(InterceptedService::new(inner, interceptor))
    }
    /// Compress requests with the given encoding.
    ///
    /// This requires the server to support it otherwise it might respond with an
    /// error.
    #[must_use]
    pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
      self.inner = self.inner.send_compressed(encoding);
      self
    }
    /// Enable decompressing responses.
    #[must_use]
    pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
      self.inner = self.inner.accept_compressed(encoding);
      self
    }
    /// Check that the plugin loaded OK. Returns the catalogue entries describing what the plugin provides
    pub async fn init_plugin(
      &mut self,
      request: impl tonic::IntoRequest<super::InitPluginRequest>,
    ) -> Result<tonic::Response<super::InitPluginResponse>, tonic::Status> {
      self.inner
        .ready()
        .await
        .map_err(|e| {
          tonic::Status::new(
            tonic::Code::Unknown,
            format!("Service was not ready: {}", e.into()),
          )
        })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static(
        "/io.pact.plugin.PactPlugin/InitPlugin",
      );
      self.inner.unary(request.into_request(), path, codec).await
    }
    /// Updated catalogue. This will be sent when the core catalogue has been updated (probably by a plugin loading).
    pub async fn update_catalogue(
      &mut self,
      request: impl tonic::IntoRequest<super::Catalogue>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
      self.inner
        .ready()
        .await
        .map_err(|e| {
          tonic::Status::new(
            tonic::Code::Unknown,
            format!("Service was not ready: {}", e.into()),
          )
        })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static(
        "/io.pact.plugin.PactPlugin/UpdateCatalogue",
      );
      self.inner.unary(request.into_request(), path, codec).await
    }
    /// Request to perform a comparison of some contents (matching request)
    pub async fn compare_contents(
      &mut self,
      request: impl tonic::IntoRequest<super::CompareContentsRequest>,
    ) -> Result<tonic::Response<super::CompareContentsResponse>, tonic::Status> {
      self.inner
        .ready()
        .await
        .map_err(|e| {
          tonic::Status::new(
            tonic::Code::Unknown,
            format!("Service was not ready: {}", e.into()),
          )
        })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static(
        "/io.pact.plugin.PactPlugin/CompareContents",
      );
      self.inner.unary(request.into_request(), path, codec).await
    }
    /// Request to configure/setup the interaction for later verification. Data returned will be persisted in the pact file.
    pub async fn configure_interaction(
      &mut self,
      request: impl tonic::IntoRequest<super::ConfigureInteractionRequest>,
    ) -> Result<
      tonic::Response<super::ConfigureInteractionResponse>,
      tonic::Status,
    > {
      self.inner
        .ready()
        .await
        .map_err(|e| {
          tonic::Status::new(
            tonic::Code::Unknown,
            format!("Service was not ready: {}", e.into()),
          )
        })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static(
        "/io.pact.plugin.PactPlugin/ConfigureInteraction",
      );
      self.inner.unary(request.into_request(), path, codec).await
    }
    /// Request to generate the content using any defined generators
    pub async fn generate_content(
      &mut self,
      request: impl tonic::IntoRequest<super::GenerateContentRequest>,
    ) -> Result<tonic::Response<super::GenerateContentResponse>, tonic::Status> {
      self.inner
        .ready()
        .await
        .map_err(|e| {
          tonic::Status::new(
            tonic::Code::Unknown,
            format!("Service was not ready: {}", e.into()),
          )
        })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static(
        "/io.pact.plugin.PactPlugin/GenerateContent",
      );
      self.inner.unary(request.into_request(), path, codec).await
    }
    /// Start a mock server
    pub async fn start_mock_server(
      &mut self,
      request: impl tonic::IntoRequest<super::StartMockServerRequest>,
    ) -> Result<tonic::Response<super::StartMockServerResponse>, tonic::Status> {
      self.inner
        .ready()
        .await
        .map_err(|e| {
          tonic::Status::new(
            tonic::Code::Unknown,
            format!("Service was not ready: {}", e.into()),
          )
        })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static(
        "/io.pact.plugin.PactPlugin/StartMockServer",
      );
      self.inner.unary(request.into_request(), path, codec).await
    }
    /// Shutdown a running mock server
    /// TODO: Replace the message types with MockServerRequest and MockServerResults in the next major version
    pub async fn shutdown_mock_server(
      &mut self,
      request: impl tonic::IntoRequest<super::ShutdownMockServerRequest>,
    ) -> Result<tonic::Response<super::ShutdownMockServerResponse>, tonic::Status> {
      self.inner
        .ready()
        .await
        .map_err(|e| {
          tonic::Status::new(
            tonic::Code::Unknown,
            format!("Service was not ready: {}", e.into()),
          )
        })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static(
        "/io.pact.plugin.PactPlugin/ShutdownMockServer",
      );
      self.inner.unary(request.into_request(), path, codec).await
    }
    /// Get the matching results from a running mock server
    pub async fn get_mock_server_results(
      &mut self,
      request: impl tonic::IntoRequest<super::MockServerRequest>,
    ) -> Result<tonic::Response<super::MockServerResults>, tonic::Status> {
      self.inner
        .ready()
        .await
        .map_err(|e| {
          tonic::Status::new(
            tonic::Code::Unknown,
            format!("Service was not ready: {}", e.into()),
          )
        })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static(
        "/io.pact.plugin.PactPlugin/GetMockServerResults",
      );
      self.inner.unary(request.into_request(), path, codec).await
    }
    /// Prepare an interaction for verification. This should return any data required to construct any request
    /// so that it can be amended before the verification is run
    pub async fn prepare_interaction_for_verification(
      &mut self,
      request: impl tonic::IntoRequest<super::VerificationPreparationRequest>,
    ) -> Result<
      tonic::Response<super::VerificationPreparationResponse>,
      tonic::Status,
    > {
      self.inner
        .ready()
        .await
        .map_err(|e| {
          tonic::Status::new(
            tonic::Code::Unknown,
            format!("Service was not ready: {}", e.into()),
          )
        })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static(
        "/io.pact.plugin.PactPlugin/PrepareInteractionForVerification",
      );
      self.inner.unary(request.into_request(), path, codec).await
    }
    /// Execute the verification for the interaction.
    pub async fn verify_interaction(
      &mut self,
      request: impl tonic::IntoRequest<super::VerifyInteractionRequest>,
    ) -> Result<tonic::Response<super::VerifyInteractionResponse>, tonic::Status> {
      self.inner
        .ready()
        .await
        .map_err(|e| {
          tonic::Status::new(
            tonic::Code::Unknown,
            format!("Service was not ready: {}", e.into()),
          )
        })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static(
        "/io.pact.plugin.PactPlugin/VerifyInteraction",
      );
      self.inner.unary(request.into_request(), path, codec).await
    }
  }
}
/// Generated server implementations.
pub mod pact_plugin_server {
  #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
  use tonic::codegen::*;
  /// Generated trait containing gRPC methods that should be implemented for use with PactPluginServer.
  #[async_trait]
  pub trait PactPlugin: Send + Sync + 'static {
    /// Check that the plugin loaded OK. Returns the catalogue entries describing what the plugin provides
    async fn init_plugin(
      &self,
      request: tonic::Request<super::InitPluginRequest>,
    ) -> Result<tonic::Response<super::InitPluginResponse>, tonic::Status>;
    /// Updated catalogue. This will be sent when the core catalogue has been updated (probably by a plugin loading).
    async fn update_catalogue(
      &self,
      request: tonic::Request<super::Catalogue>,
    ) -> Result<tonic::Response<()>, tonic::Status>;
    /// Request to perform a comparison of some contents (matching request)
    async fn compare_contents(
      &self,
      request: tonic::Request<super::CompareContentsRequest>,
    ) -> Result<tonic::Response<super::CompareContentsResponse>, tonic::Status>;
    /// Request to configure/setup the interaction for later verification. Data returned will be persisted in the pact file.
    async fn configure_interaction(
      &self,
      request: tonic::Request<super::ConfigureInteractionRequest>,
    ) -> Result<tonic::Response<super::ConfigureInteractionResponse>, tonic::Status>;
    /// Request to generate the content using any defined generators
    async fn generate_content(
      &self,
      request: tonic::Request<super::GenerateContentRequest>,
    ) -> Result<tonic::Response<super::GenerateContentResponse>, tonic::Status>;
    /// Start a mock server
    async fn start_mock_server(
      &self,
      request: tonic::Request<super::StartMockServerRequest>,
    ) -> Result<tonic::Response<super::StartMockServerResponse>, tonic::Status>;
    /// Shutdown a running mock server
    /// TODO: Replace the message types with MockServerRequest and MockServerResults in the next major version
    async fn shutdown_mock_server(
      &self,
      request: tonic::Request<super::ShutdownMockServerRequest>,
    ) -> Result<tonic::Response<super::ShutdownMockServerResponse>, tonic::Status>;
    /// Get the matching results from a running mock server
    async fn get_mock_server_results(
      &self,
      request: tonic::Request<super::MockServerRequest>,
    ) -> Result<tonic::Response<super::MockServerResults>, tonic::Status>;
    /// Prepare an interaction for verification. This should return any data required to construct any request
    /// so that it can be amended before the verification is run
    async fn prepare_interaction_for_verification(
      &self,
      request: tonic::Request<super::VerificationPreparationRequest>,
    ) -> Result<
      tonic::Response<super::VerificationPreparationResponse>,
      tonic::Status,
    >;
    /// Execute the verification for the interaction.
    async fn verify_interaction(
      &self,
      request: tonic::Request<super::VerifyInteractionRequest>,
    ) -> Result<tonic::Response<super::VerifyInteractionResponse>, tonic::Status>;
  }
  #[derive(Debug)]
  pub struct PactPluginServer<T: PactPlugin> {
    inner: _Inner<T>,
    accept_compression_encodings: EnabledCompressionEncodings,
    send_compression_encodings: EnabledCompressionEncodings,
  }
  struct _Inner<T>(Arc<T>);
  impl<T: PactPlugin> PactPluginServer<T> {
    pub fn new(inner: T) -> Self {
      Self::from_arc(Arc::new(inner))
    }
    pub fn from_arc(inner: Arc<T>) -> Self {
      let inner = _Inner(inner);
      Self {
        inner,
        accept_compression_encodings: Default::default(),
        send_compression_encodings: Default::default(),
      }
    }
    pub fn with_interceptor<F>(
      inner: T,
      interceptor: F,
    ) -> InterceptedService<Self, F>
      where
        F: tonic::service::Interceptor,
    {
      InterceptedService::new(Self::new(inner), interceptor)
    }
    /// Enable decompressing requests with the given encoding.
    #[must_use]
    pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
      self.accept_compression_encodings.enable(encoding);
      self
    }
    /// Compress responses with the given encoding, if the client supports it.
    #[must_use]
    pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
      self.send_compression_encodings.enable(encoding);
      self
    }
  }
  impl<T, B> tonic::codegen::Service<http::Request<B>> for PactPluginServer<T>
    where
      T: PactPlugin,
      B: Body + Send + 'static,
      B::Error: Into<StdError> + Send + 'static,
  {
    type Response = http::Response<tonic::body::BoxBody>;
    type Error = std::convert::Infallible;
    type Future = BoxFuture<Self::Response, Self::Error>;
    fn poll_ready(
      &mut self,
      _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
      Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: http::Request<B>) -> Self::Future {
      let inner = self.inner.clone();
      match req.uri().path() {
        "/io.pact.plugin.PactPlugin/InitPlugin" => {
          #[allow(non_camel_case_types)]
          struct InitPluginSvc<T: PactPlugin>(pub Arc<T>);
          impl<
            T: PactPlugin,
          > tonic::server::UnaryService<super::InitPluginRequest>
          for InitPluginSvc<T> {
            type Response = super::InitPluginResponse;
            type Future = BoxFuture<
              tonic::Response<Self::Response>,
              tonic::Status,
            >;
            fn call(
              &mut self,
              request: tonic::Request<super::InitPluginRequest>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move { (*inner).init_plugin(request).await };
              Box::pin(fut)
            }
          }
          let accept_compression_encodings = self.accept_compression_encodings;
          let send_compression_encodings = self.send_compression_encodings;
          let inner = self.inner.clone();
          let fut = async move {
            let inner = inner.0;
            let method = InitPluginSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = tonic::server::Grpc::new(codec)
              .apply_compression_config(
                accept_compression_encodings,
                send_compression_encodings,
              );
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        "/io.pact.plugin.PactPlugin/UpdateCatalogue" => {
          #[allow(non_camel_case_types)]
          struct UpdateCatalogueSvc<T: PactPlugin>(pub Arc<T>);
          impl<T: PactPlugin> tonic::server::UnaryService<super::Catalogue>
          for UpdateCatalogueSvc<T> {
            type Response = ();
            type Future = BoxFuture<
              tonic::Response<Self::Response>,
              tonic::Status,
            >;
            fn call(
              &mut self,
              request: tonic::Request<super::Catalogue>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move {
                (*inner).update_catalogue(request).await
              };
              Box::pin(fut)
            }
          }
          let accept_compression_encodings = self.accept_compression_encodings;
          let send_compression_encodings = self.send_compression_encodings;
          let inner = self.inner.clone();
          let fut = async move {
            let inner = inner.0;
            let method = UpdateCatalogueSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = tonic::server::Grpc::new(codec)
              .apply_compression_config(
                accept_compression_encodings,
                send_compression_encodings,
              );
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        "/io.pact.plugin.PactPlugin/CompareContents" => {
          #[allow(non_camel_case_types)]
          struct CompareContentsSvc<T: PactPlugin>(pub Arc<T>);
          impl<
            T: PactPlugin,
          > tonic::server::UnaryService<super::CompareContentsRequest>
          for CompareContentsSvc<T> {
            type Response = super::CompareContentsResponse;
            type Future = BoxFuture<
              tonic::Response<Self::Response>,
              tonic::Status,
            >;
            fn call(
              &mut self,
              request: tonic::Request<super::CompareContentsRequest>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move {
                (*inner).compare_contents(request).await
              };
              Box::pin(fut)
            }
          }
          let accept_compression_encodings = self.accept_compression_encodings;
          let send_compression_encodings = self.send_compression_encodings;
          let inner = self.inner.clone();
          let fut = async move {
            let inner = inner.0;
            let method = CompareContentsSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = tonic::server::Grpc::new(codec)
              .apply_compression_config(
                accept_compression_encodings,
                send_compression_encodings,
              );
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        "/io.pact.plugin.PactPlugin/ConfigureInteraction" => {
          #[allow(non_camel_case_types)]
          struct ConfigureInteractionSvc<T: PactPlugin>(pub Arc<T>);
          impl<
            T: PactPlugin,
          > tonic::server::UnaryService<super::ConfigureInteractionRequest>
          for ConfigureInteractionSvc<T> {
            type Response = super::ConfigureInteractionResponse;
            type Future = BoxFuture<
              tonic::Response<Self::Response>,
              tonic::Status,
            >;
            fn call(
              &mut self,
              request: tonic::Request<super::ConfigureInteractionRequest>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move {
                (*inner).configure_interaction(request).await
              };
              Box::pin(fut)
            }
          }
          let accept_compression_encodings = self.accept_compression_encodings;
          let send_compression_encodings = self.send_compression_encodings;
          let inner = self.inner.clone();
          let fut = async move {
            let inner = inner.0;
            let method = ConfigureInteractionSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = tonic::server::Grpc::new(codec)
              .apply_compression_config(
                accept_compression_encodings,
                send_compression_encodings,
              );
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        "/io.pact.plugin.PactPlugin/GenerateContent" => {
          #[allow(non_camel_case_types)]
          struct GenerateContentSvc<T: PactPlugin>(pub Arc<T>);
          impl<
            T: PactPlugin,
          > tonic::server::UnaryService<super::GenerateContentRequest>
          for GenerateContentSvc<T> {
            type Response = super::GenerateContentResponse;
            type Future = BoxFuture<
              tonic::Response<Self::Response>,
              tonic::Status,
            >;
            fn call(
              &mut self,
              request: tonic::Request<super::GenerateContentRequest>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move {
                (*inner).generate_content(request).await
              };
              Box::pin(fut)
            }
          }
          let accept_compression_encodings = self.accept_compression_encodings;
          let send_compression_encodings = self.send_compression_encodings;
          let inner = self.inner.clone();
          let fut = async move {
            let inner = inner.0;
            let method = GenerateContentSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = tonic::server::Grpc::new(codec)
              .apply_compression_config(
                accept_compression_encodings,
                send_compression_encodings,
              );
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        "/io.pact.plugin.PactPlugin/StartMockServer" => {
          #[allow(non_camel_case_types)]
          struct StartMockServerSvc<T: PactPlugin>(pub Arc<T>);
          impl<
            T: PactPlugin,
          > tonic::server::UnaryService<super::StartMockServerRequest>
          for StartMockServerSvc<T> {
            type Response = super::StartMockServerResponse;
            type Future = BoxFuture<
              tonic::Response<Self::Response>,
              tonic::Status,
            >;
            fn call(
              &mut self,
              request: tonic::Request<super::StartMockServerRequest>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move {
                (*inner).start_mock_server(request).await
              };
              Box::pin(fut)
            }
          }
          let accept_compression_encodings = self.accept_compression_encodings;
          let send_compression_encodings = self.send_compression_encodings;
          let inner = self.inner.clone();
          let fut = async move {
            let inner = inner.0;
            let method = StartMockServerSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = tonic::server::Grpc::new(codec)
              .apply_compression_config(
                accept_compression_encodings,
                send_compression_encodings,
              );
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        "/io.pact.plugin.PactPlugin/ShutdownMockServer" => {
          #[allow(non_camel_case_types)]
          struct ShutdownMockServerSvc<T: PactPlugin>(pub Arc<T>);
          impl<
            T: PactPlugin,
          > tonic::server::UnaryService<super::ShutdownMockServerRequest>
          for ShutdownMockServerSvc<T> {
            type Response = super::ShutdownMockServerResponse;
            type Future = BoxFuture<
              tonic::Response<Self::Response>,
              tonic::Status,
            >;
            fn call(
              &mut self,
              request: tonic::Request<super::ShutdownMockServerRequest>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move {
                (*inner).shutdown_mock_server(request).await
              };
              Box::pin(fut)
            }
          }
          let accept_compression_encodings = self.accept_compression_encodings;
          let send_compression_encodings = self.send_compression_encodings;
          let inner = self.inner.clone();
          let fut = async move {
            let inner = inner.0;
            let method = ShutdownMockServerSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = tonic::server::Grpc::new(codec)
              .apply_compression_config(
                accept_compression_encodings,
                send_compression_encodings,
              );
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        "/io.pact.plugin.PactPlugin/GetMockServerResults" => {
          #[allow(non_camel_case_types)]
          struct GetMockServerResultsSvc<T: PactPlugin>(pub Arc<T>);
          impl<
            T: PactPlugin,
          > tonic::server::UnaryService<super::MockServerRequest>
          for GetMockServerResultsSvc<T> {
            type Response = super::MockServerResults;
            type Future = BoxFuture<
              tonic::Response<Self::Response>,
              tonic::Status,
            >;
            fn call(
              &mut self,
              request: tonic::Request<super::MockServerRequest>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move {
                (*inner).get_mock_server_results(request).await
              };
              Box::pin(fut)
            }
          }
          let accept_compression_encodings = self.accept_compression_encodings;
          let send_compression_encodings = self.send_compression_encodings;
          let inner = self.inner.clone();
          let fut = async move {
            let inner = inner.0;
            let method = GetMockServerResultsSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = tonic::server::Grpc::new(codec)
              .apply_compression_config(
                accept_compression_encodings,
                send_compression_encodings,
              );
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        "/io.pact.plugin.PactPlugin/PrepareInteractionForVerification" => {
          #[allow(non_camel_case_types)]
          struct PrepareInteractionForVerificationSvc<T: PactPlugin>(
            pub Arc<T>,
          );
          impl<
            T: PactPlugin,
          > tonic::server::UnaryService<super::VerificationPreparationRequest>
          for PrepareInteractionForVerificationSvc<T> {
            type Response = super::VerificationPreparationResponse;
            type Future = BoxFuture<
              tonic::Response<Self::Response>,
              tonic::Status,
            >;
            fn call(
              &mut self,
              request: tonic::Request<
                super::VerificationPreparationRequest,
              >,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move {
                (*inner).prepare_interaction_for_verification(request).await
              };
              Box::pin(fut)
            }
          }
          let accept_compression_encodings = self.accept_compression_encodings;
          let send_compression_encodings = self.send_compression_encodings;
          let inner = self.inner.clone();
          let fut = async move {
            let inner = inner.0;
            let method = PrepareInteractionForVerificationSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = tonic::server::Grpc::new(codec)
              .apply_compression_config(
                accept_compression_encodings,
                send_compression_encodings,
              );
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        "/io.pact.plugin.PactPlugin/VerifyInteraction" => {
          #[allow(non_camel_case_types)]
          struct VerifyInteractionSvc<T: PactPlugin>(pub Arc<T>);
          impl<
            T: PactPlugin,
          > tonic::server::UnaryService<super::VerifyInteractionRequest>
          for VerifyInteractionSvc<T> {
            type Response = super::VerifyInteractionResponse;
            type Future = BoxFuture<
              tonic::Response<Self::Response>,
              tonic::Status,
            >;
            fn call(
              &mut self,
              request: tonic::Request<super::VerifyInteractionRequest>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move {
                (*inner).verify_interaction(request).await
              };
              Box::pin(fut)
            }
          }
          let accept_compression_encodings = self.accept_compression_encodings;
          let send_compression_encodings = self.send_compression_encodings;
          let inner = self.inner.clone();
          let fut = async move {
            let inner = inner.0;
            let method = VerifyInteractionSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = tonic::server::Grpc::new(codec)
              .apply_compression_config(
                accept_compression_encodings,
                send_compression_encodings,
              );
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        _ => {
          Box::pin(async move {
            Ok(
              http::Response::builder()
                .status(200)
                .header("grpc-status", "12")
                .header("content-type", "application/grpc")
                .body(empty_body())
                .unwrap(),
            )
          })
        }
      }
    }
  }
  impl<T: PactPlugin> Clone for PactPluginServer<T> {
    fn clone(&self) -> Self {
      let inner = self.inner.clone();
      Self {
        inner,
        accept_compression_encodings: self.accept_compression_encodings,
        send_compression_encodings: self.send_compression_encodings,
      }
    }
  }
  impl<T: PactPlugin> Clone for _Inner<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }
  impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "{:?}", self.0)
    }
  }
  impl<T: PactPlugin> tonic::server::NamedService for PactPluginServer<T> {
    const NAME: &'static str = "io.pact.plugin.PactPlugin";
  }
}

// ------------------ Generated --------------------------//

impl From<&OptionalBody> for Body {
  fn from(body: &OptionalBody) -> Self {
    match body {
      OptionalBody::Present(bytes, ct, ct_hint) => Body {
        content_type: ct.as_ref().map(|ct| ct.to_string()).unwrap_or_default(),
        content: Some(bytes.to_vec()),
        content_type_hint: match ct_hint {
          Some(ct_hint) => match ct_hint {
            ContentTypeHint::BINARY => body::ContentTypeHint::Binary as i32,
            ContentTypeHint::TEXT => body::ContentTypeHint::Text as i32,
            ContentTypeHint::DEFAULT => body::ContentTypeHint::Default as i32
          }
          None => body::ContentTypeHint::Default as i32
        }
      },
      _ => Body {
        content_type: "".to_string(),
        content: None,
        content_type_hint: body::ContentTypeHint::Default as i32
      }
    }
  }
}
