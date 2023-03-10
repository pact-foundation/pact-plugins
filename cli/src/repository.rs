use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::RepositoryCommands;

pub(crate) fn handle_command(repository_command: &RepositoryCommands) -> anyhow::Result<()> {
  match repository_command {
    RepositoryCommands::Validate => {
      todo!()
    }
    RepositoryCommands::New { filename, overwrite } => new_repository(filename, *overwrite),
    RepositoryCommands::AddPluginVersion => { todo!() }
    RepositoryCommands::AddAllPluginVersions => { todo!() }
    RepositoryCommands::YankVersion => { todo!() }
    RepositoryCommands::List => { todo!() }
    RepositoryCommands::ListVersions => { todo!() }
  }
}

/// Struct representing the plugin repository index file
#[derive(Serialize, Deserialize, Debug)]
struct PluginRepositoryIndex {
  /// Version of this index file
  index_version: usize,

  /// File format version of the index file
  format_version: usize,

  /// Timestamp (in UTC) that the file was created/updated
  timestamp: DateTime<Utc>,

  /// Plugin entries
  entries: HashMap<String, PluginEntry>
}

/// Struct to store the plugin version entries
#[derive(Serialize, Deserialize, Debug)]
struct PluginEntry {

}

impl Default for PluginRepositoryIndex {
  fn default() -> Self {
    PluginRepositoryIndex {
      index_version: 0,
      format_version: 0,
      timestamp: Utc::now(),
      entries: Default::default()
    }
  }
}

/// Create a new repository file
fn new_repository(filename: &Option<String>, overwrite: bool) -> anyhow::Result<()> {
  let filename = filename.clone().unwrap_or("repository.index".to_string());
  let path = PathBuf::from(filename.as_str());
  let abs_path = path.canonicalize().unwrap_or(path.clone());
  if path.exists() && !overwrite {
    Err(anyhow!("Repository file '{}' already exists and overwrite was not specified", abs_path.to_string_lossy()))
  } else {
    if let Some(parent) = path.parent() {
      if !parent.exists() {
        info!(?parent, "Parent directory does not exist, creating it");
        fs::create_dir_all(parent.clone())?;
      }
    }

    let repository = PluginRepositoryIndex {
      .. PluginRepositoryIndex::default()
    };
    let toml = toml::to_string(&repository)?;
    let mut f = File::create(path.clone())?;
    f.write_all(toml.as_bytes())?;

    println!("Created new blank repository file '{}'", abs_path.to_string_lossy());

    Ok(())
  }
}
