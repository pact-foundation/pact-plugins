//! Pact plugin driver library for Rust

/// Recommended `tracing`/`log` filter directives to suppress gRPC transport-layer trace
/// noise (h2 frame encoding, tonic channel reconnects, hyper connection pool chatter).
///
/// Add this to `RUST_LOG` or compose it into your `EnvFilter` in the test harness:
/// ```text
/// RUST_LOG=warn,my_crate=debug,pact_plugin_driver=info,h2=warn,hyper=warn,hyper_util=warn,tonic=warn,tower=warn
/// ```
/// Or in code:
/// ```rust,ignore
/// use tracing_subscriber::EnvFilter;
/// let filter = EnvFilter::new(format!("info,{}", pact_plugin_driver::TRANSPORT_FILTER_DIRECTIVES));
/// ```
pub const TRANSPORT_FILTER_DIRECTIVES: &str = "h2=warn,hyper=warn,hyper_util=warn,tonic=warn,tower=warn";

pub mod call_chain;
pub mod catalogue_manager;
mod child_process;
pub mod core_capabilities;
pub mod grpc_plugin;
pub(crate) mod plugin_host;
pub mod content;
pub mod download;
pub mod field;
#[cfg(feature = "lua")]
pub mod lua_plugin;
mod metrics;
pub mod mock_server;
pub mod plugin_log_sink;
pub mod plugin_manager;
pub mod plugin_models;
pub mod proto;
// Public because the V2-only message types are part of the driver's public API: the field-level
// operations (proposal 006) have no V1 equivalent, so the embedding Pact framework needs these
// types to implement `core_capabilities::CoreFieldMatcher`/`CoreFieldGenerator`.
pub mod proto_v2;
pub mod repository;
pub mod test_context;
pub mod utils;
pub mod verification;
