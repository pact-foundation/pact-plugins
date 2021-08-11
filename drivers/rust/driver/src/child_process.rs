//! Module for managing running child processes

use std::time::Duration;

use anyhow::anyhow;
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use duct::ReaderHandle;
use std::sync::mpsc::channel;

use crate::plugin_models::PactPluginManifest;
use std::io::BufRead;
use std::io::BufReader;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RunningPluginInfo {
  pub port: u16,
  pub server_key: String
}

/// Running child process
#[derive(Debug, Clone)]
pub struct ChildPluginProcess {
  child_pid: u32,
  manifest: PactPluginManifest,
  plugin_info: RunningPluginInfo
}

impl ChildPluginProcess {
  /// Start the child process and try read the startup JSON message from its standard output.
  pub fn new(child: ReaderHandle, manifest: &PactPluginManifest) -> anyhow::Result<Self> {
    let (tx, rx) = channel();
    let manifest = manifest.clone();
    let plugin_name = manifest.name.clone();
    let child_pid = child.pids().first().cloned().unwrap_or_default();

    tokio::task::spawn_blocking(move || {
      let buffer = BufReader::new(child);
      let mut startup_read = false;
      for line in buffer.lines() {
        match line {
          Ok(line) => {
            debug!("Plugin {} - {}", plugin_name, line);
            if !startup_read {
              let line = line.trim();
              if line.starts_with("{") {
                startup_read = true;
                match serde_json::from_str::<RunningPluginInfo>(line) {
                  Ok(plugin_info) => {
                    tx.send(Ok(ChildPluginProcess {
                      child_pid: child_pid,
                      manifest: manifest.clone(),
                      plugin_info
                    }))
                  }
                  Err(err) => {
                    error!("Failed to read startup info from plugin - {}", err);
                    tx.send(Err(anyhow!("Failed to read startup info from plugin - {}", err)))
                  }
                }.unwrap_or_default();
              }
            }
          }
          Err(err) => warn!("Failed to read line from child process output - {}", err)
        };
      }
    });

    match rx.recv_timeout(Duration::from_millis(500)) {
      Ok(result) => result,
      Err(err) => {
        error!("Timeout waiting to get plugin startup info - {}", err);
        Err(anyhow!("Plugin process did not output the correct startup message in 500 ms"))
      }
    }
  }

  /// Port the plugin is running on
  pub fn port(&self) -> u16 {
    self.plugin_info.port
  }
}
