//! Driver-level plugin log sink abstraction

use std::sync::RwLock;

use lazy_static::lazy_static;

/// Source of a plugin log entry
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginLogSource {
  /// Raw line read from the plugin process stderr
  Stderr,
  /// Structured record received via the PluginHost Log RPC
  LogRpc,
}

/// Structured log entry produced by a running plugin
#[derive(Debug, Clone)]
pub struct PluginLogEntry {
  /// Plugin name from its manifest
  pub plugin_name: String,
  /// UUID assigned by the driver when this plugin instance was started
  pub plugin_instance_id: String,
  /// Test run ID extracted from testContext, if available
  pub test_run_id: Option<String>,
  /// Log level string: TRACE, DEBUG, INFO, WARN, ERROR
  pub level: String,
  /// Human-readable log message
  pub message: String,
  /// Logger name / module path, if known
  pub target: Option<String>,
  /// Unix epoch milliseconds
  pub timestamp_ms: i64,
  /// Where this entry originated
  pub source: PluginLogSource,
}

/// Receives structured log entries from running plugin processes.
///
/// Register a custom implementation with [`set_plugin_log_sink`] to intercept plugin log
/// output. The default sink is a no-op: stderr is already written to the per-instance log
/// file by the driver, and Log RPC entries will be forwarded to `tracing` once the
/// `PluginHost` server is implemented.
pub trait PluginLogSink: Send + Sync {
  fn log(&self, entry: &PluginLogEntry);
}

struct DefaultPluginLogSink;

impl PluginLogSink for DefaultPluginLogSink {
  fn log(&self, _entry: &PluginLogEntry) {}
}

lazy_static! {
  static ref PLUGIN_LOG_SINK: RwLock<Box<dyn PluginLogSink>> =
    RwLock::new(Box::new(DefaultPluginLogSink));
}

/// Replace the active plugin log sink. Should be called once at startup before any plugins load.
pub fn set_plugin_log_sink(sink: Box<dyn PluginLogSink>) {
  *PLUGIN_LOG_SINK.write().unwrap() = sink;
}

/// Forward a log entry to the registered sink. Called by driver internals.
pub(crate) fn emit_plugin_log(entry: &PluginLogEntry) {
  PLUGIN_LOG_SINK.read().unwrap().log(entry);
}
