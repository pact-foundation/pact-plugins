//! Module for managing running child processes
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Child;
use std::sync::mpsc::channel;
use std::time::Duration;

use anyhow::anyhow;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, System};
use tracing::{debug, error, trace, warn};

use crate::plugin_log_sink::{PluginLogEntry, PluginLogSource, emit_plugin_log};
use crate::plugin_manager::pact_plugin_dir;
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
  pub plugin_info: RunningPluginInfo,
  /// UUID assigned by the driver at process start
  pub instance_id: String,
}

fn plugin_log_dir() -> PathBuf {
  if let Ok(dir) = std::env::var("PACT_OUTPUT_DIR") {
    PathBuf::from(dir).join("logs")
  } else {
    pact_plugin_dir()
      .unwrap_or_else(|_| PathBuf::from("."))
      .join("logs")
  }
}

fn open_plugin_log_file(plugin_name: &str, instance_id: &str) -> Option<File> {
  let log_dir = plugin_log_dir();
  if let Err(err) = fs::create_dir_all(&log_dir) {
    warn!("Could not create plugin log directory {:?}: {}", log_dir, err);
    return None;
  }
  let log_path = log_dir.join(format!("pact-plugin-{}-{}.log", plugin_name, instance_id));
  match File::create(&log_path) {
    Ok(f) => {
      debug!("Plugin stderr for instance {} captured to {:?}", instance_id, log_path);
      Some(f)
    }
    Err(err) => {
      warn!("Could not create plugin log file {:?}: {}", log_path, err);
      None
    }
  }
}

impl ChildPluginProcess {
  /// Start the child process and try read the startup JSON message from its standard output.
  pub async fn new(mut child: Child, manifest: &PactPluginManifest, instance_id: String) -> anyhow::Result<Self> {
    let (tx, rx) = channel();
    let child_pid = child.id();
    let child_out = child.stdout.take()
      .ok_or_else(|| anyhow!("Could not get the child process standard output stream"))?;
    let child_err = child.stderr.take()
      .ok_or_else(|| anyhow!("Could not get the child process standard error stream"))?;

    trace!("Starting output polling tasks...");

    let plugin_name = manifest.name.clone();
    let stdout_instance_id = instance_id.clone();
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
                    plugin_info,
                    instance_id: stdout_instance_id.clone(),
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
        line.clear();
      }
      trace!("Thread to poll plugin STDOUT done");
    });

    let log_file = open_plugin_log_file(&manifest.name, &instance_id);
    let plugin_name = manifest.name.clone();
    let stderr_instance_id = instance_id;
    std::thread::spawn(move || {
      trace!("Starting thread to poll plugin STDERR");
      let mut log_file = log_file;
      let mut reader = BufReader::new(child_err);
      let mut line = String::with_capacity(256);
      while let Ok(chars_read) = reader.read_line(&mut line) {
        if chars_read > 0 {
          if let Some(ref mut f) = log_file {
            let _ = f.write_all(line.as_bytes());
          }
          emit_plugin_log(&PluginLogEntry {
            plugin_name: plugin_name.clone(),
            plugin_instance_id: stderr_instance_id.clone(),
            test_run_id: None,
            level: "DEBUG".to_string(),
            message: line.trim_end_matches(['\n', '\r']).to_string(),
            target: None,
            timestamp_ms: Utc::now().timestamp_millis(),
            source: PluginLogSource::Stderr,
          });
        } else {
          trace!("0 bytes read from STDERR, this indicates EOF");
          break;
        }
        line.clear();
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
      #[cfg(not(windows))]
      process.kill();
      // revert windows specific logic once https://github.com/GuillaumeGomez/sysinfo/pull/1341/files is merged/released
      #[cfg(windows)]
      let _ = std::process::Command::new("taskkill.exe").arg("/PID").arg(self.child_pid.to_string()).arg("/F").arg("/T").output();
    } else {
      warn!("Child process with PID {} was not found", self.child_pid);
    }
  }
}
