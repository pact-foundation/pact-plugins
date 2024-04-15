//! Module for managing running child processes (Windows version)
//!
//! This uses threads to read STDOUT/STDERR from the plugin process instead of Tokio tasks.
//!
use std::io::{BufRead, BufReader};
use std::process::Child;
use std::sync::mpsc::channel;
use std::time::Duration;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, Signal, System};
use tracing::{debug, error, trace, warn};

use crate::plugin_models::PactPluginManifest;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RunningPluginInfo {
  pub port: u16,
  pub server_key: String
}

/// Running child process
#[derive(Debug, Clone)]
pub struct ChildPluginProcess {
  /// OS PID of the running process
  pub child_pid: usize,
  /// Info on the running plugin
  pub plugin_info: RunningPluginInfo
}

impl ChildPluginProcess {
  /// Start the child process and try read the startup JSON message from its standard output.
  pub async fn new(mut child: Child, manifest: &PactPluginManifest) -> anyhow::Result<Self> {
    let (tx, rx) = channel();
    let child_pid = child.id();
    let child_out = child.stdout.take()
      .ok_or_else(|| anyhow!("Could not get the child process standard output stream"))?;
    let child_err = child.stderr.take()
      .ok_or_else(|| anyhow!("Could not get the child process standard error stream"))?;

    trace!("Starting output polling tasks...");

    let plugin_name = manifest.name.clone();
    std::thread::spawn(move || {
      trace!("Starting thread to poll plugin STDOUT");

      let mut startup_read = false;
      let mut reader = BufReader::new(child_out);
      let mut line = String::with_capacity(256);
      while let Ok(chars_read) = reader.read_line(&mut line) {
        if chars_read > 0 {
          debug!("Plugin({}, {}, STDOUT) || {}", plugin_name, child_pid, line);
          if !startup_read {
            let line = line.trim();
            if line.starts_with("{") {
              match serde_json::from_str::<RunningPluginInfo>(line) {
                Ok(plugin_info) => {
                  tx.send(Ok(ChildPluginProcess {
                    child_pid: child_pid as usize,
                    plugin_info
                  })).unwrap_or_default()
                }
                Err(err) => {
                  error!("Failed to read startup info from plugin - {}", err);
                  tx.send(Err(anyhow!("Failed to read startup info from plugin - {}", err)))
                    .unwrap_or_default()
                }
              };
              startup_read = true;
            }
          }
        } else {
          trace!("0 bytes read from STDOUT, this indicates EOF");
          break;
        }
      }
      trace!("Thread to poll plugin STDOUT done");
    });

    let plugin_name = manifest.name.clone();
    std::thread::spawn(move || {
      trace!("Starting thread to poll plugin STDERR");
      let mut reader = BufReader::new(child_err);
      let mut line = String::with_capacity(256);
      while let Ok(chars_read) = reader.read_line(&mut line) {
        if chars_read > 0 {
          debug!("Plugin({}, {}, STDERR) || {}", plugin_name, child_pid, line);
        } else {
          trace!("0 bytes read from STDERR, this indicates EOF");
          break;
        }
      }
      trace!("Thread to poll plugin STDERR done");
    });

    trace!("Starting output polling tasks... DONE");

    // TODO: This timeout needs to be configurable
    // TODO: Timeout is not working on Alpine, waits indefinitely if the plugin does not start properly
    match rx.recv_timeout(Duration::from_secs(60)) {
      Ok(value) => value,
      Err(err) => {
        error!("Timeout waiting to get plugin startup info: {}", err);
        Err(anyhow!("Plugin process did not output the correct startup message in 60 seconds: {}", err))
      }
    }
  }

  /// Port the plugin is running on
  pub fn port(&self) -> u16 {
    self.plugin_info.port
  }

  /// Kill the running plugin process
  pub fn kill(&self) {
    let mut s = System::new();
    s.refresh_processes();
    if let Some(process) = s.process(Pid::from_u32(self.child_pid as u32)) {
      process.kill_with(Signal::Term);
    } else {
      warn!("Child process with PID {} was not found", self.child_pid);
    }
  }
}
