//! Module for managing running child processes

use anyhow::anyhow;
use log::{debug, error, trace, warn};
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessExt, RefreshKind, Signal, System, SystemExt};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc::channel;

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
  child_pid: usize,
  manifest: PactPluginManifest,
  plugin_info: RunningPluginInfo
}

impl ChildPluginProcess {
  /// Start the child process and try read the startup JSON message from its standard output.
  pub async fn new(mut child: Child, manifest: &PactPluginManifest) -> anyhow::Result<Self> {
    let (tx, mut rx) = channel(1);
    let child_pid = child.id()
      .ok_or_else(|| anyhow!("Could not get the child process ID"))?;
    let child_out = child.stdout.take()
      .ok_or_else(|| anyhow!("Could not get the child process standard output stream"))?;
    let child_err = child.stderr.take()
      .ok_or_else(|| anyhow!("Could not get the child process standard error stream"))?;

    let mfso = manifest.clone();
    tokio::task::spawn(async move {
      trace!("Starting task to poll plugin stdout");
      let mut startup_read = false;
      let reader = BufReader::new(child_out);
      let mut lines = reader.lines();
      let plugin_name = mfso.name.as_str();
      while let Ok(line) = lines.next_line().await {
        if let Some(line) = line {
          debug!("Plugin({}, {}, STDOUT): {}", plugin_name, child_pid, line);
          if !startup_read {
            let line = line.trim();
            if line.starts_with("{") {
              startup_read = true;
              match serde_json::from_str::<RunningPluginInfo>(line) {
                Ok(plugin_info) => {
                  tx.send(Ok(ChildPluginProcess {
                    child_pid: child_pid as usize,
                    manifest: mfso.clone(),
                    plugin_info
                  })).await.unwrap_or_default()
                }
                Err(err) => {
                  error!("Failed to read startup info from plugin - {}", err);
                  tx.send(Err(anyhow!("Failed to read startup info from plugin - {}", err)))
                    .await.unwrap_or_default()
                }
              }
            }
          }
        }
      }
      trace!("Task to poll plugin stderr done");
    });

    let plugin_name = manifest.name.clone();
    tokio::task::spawn(async move {
      trace!("Starting task to poll plugin stderr");
      let reader = BufReader::new(child_err);
      let mut lines = reader.lines();
      while let Ok(line) = lines.next_line().await {
        if let Some(line) = line {
          debug!("Plugin({}, {}, STDERR): {}", plugin_name, child_pid, line);
        }
      }
      trace!("Task to poll plugin stderr done");
    });

    match rx.recv().await {
      Some(result) => result,
      None => {
        error!("Timeout waiting to get plugin startup info");
        Err(anyhow!("Plugin process did not output the correct startup message"))
      }
    }
  }

  /// Port the plugin is running on
  pub fn port(&self) -> u16 {
    self.plugin_info.port
  }

  /// Kill the running plugin process
  pub fn kill(&self) {
    let s = System::new_with_specifics(RefreshKind::new().with_processes());
    if let Some(process) = s.process(self.child_pid as Pid) {
      process.kill(Signal::Term);
    } else {
      warn!("Child process with PID {} was not found", self.child_pid);
    }
  }
}
