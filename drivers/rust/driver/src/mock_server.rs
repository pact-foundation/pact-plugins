//! Module to support dealing with mock servers from plugins

use std::path::PathBuf;

use crate::content::ContentMismatch;
use crate::plugin_models::PactPlugin;

/// Mock server configuration
#[derive(Debug, Default, Clone)]
pub struct MockServerConfig {
  /// Output path to generate Pact files to. Defaults to the current working directory.
  pub output_path: Option<PathBuf>,
  /// Host interface to use to bind to. Defaults to the loopback adapter.
  pub host_interface: Option<String>,
  /// Port to bind to. Default (or a value of 0) get the OS to open a random port
  pub port: u32,
  /// If TLS should be used (if supported by the mock server)
  pub tls: bool
}

/// Details of the running mock server
#[derive(Debug, Clone)]
pub struct MockServerDetails {
  /// Unique key for the mock server
  pub key: String,
  /// Base URL to the running mock server
  pub base_url: String,
  /// Port the mock server is running on
  pub port: u32,
  /// Plugin the mock server belongs to
  pub plugin: PactPlugin
}

/// Results from the mock server
#[derive(Debug, Default, Clone)]
pub struct MockServerResults {
  /// service + method that was requested
  pub path: String,
  /// If an error occurred trying to handle the request
  pub error: String,
  /// Any mismatches that occurred
  pub mismatches: Vec<ContentMismatch>
}
