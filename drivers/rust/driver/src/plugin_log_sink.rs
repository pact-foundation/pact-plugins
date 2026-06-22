//! Driver-level plugin log sink abstraction

use std::sync::RwLock;

use lazy_static::lazy_static;
use tracing::{debug, error, info, warn};

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
/// output. The built-in `DefaultPluginLogSink` is a no-op for [`PluginLogSource::Stderr`]
/// entries (those are already written to the per-instance log file) and forwards
/// [`PluginLogSource::LogRpc`] entries into the `tracing` subscriber.
pub trait PluginLogSink: Send + Sync {
  fn log(&self, entry: &PluginLogEntry);
}

struct DefaultPluginLogSink;

impl PluginLogSink for DefaultPluginLogSink {
  fn log(&self, entry: &PluginLogEntry) {
    if entry.source != PluginLogSource::LogRpc {
      return;
    }
    if entry.level.to_uppercase() == "TRACE" {
      return;
    }
    let plugin = &entry.plugin_name;
    let instance = &entry.plugin_instance_id;
    let msg = &entry.message;
    let run_id = entry.test_run_id.as_deref().unwrap_or("");
    match entry.level.to_uppercase().as_str() {
      "DEBUG" => debug!(plugin_name = %plugin, plugin_instance = %instance, test_run_id = %run_id, "{}", msg),
      "INFO"  => info!(plugin_name = %plugin, plugin_instance = %instance, test_run_id = %run_id, "{}", msg),
      "WARN"  => warn!(plugin_name = %plugin, plugin_instance = %instance, test_run_id = %run_id, "{}", msg),
      _       => error!(plugin_name = %plugin, plugin_instance = %instance, test_run_id = %run_id, "{}", msg),
    }
  }
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
