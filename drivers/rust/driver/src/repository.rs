//! Module for dealing with the plugin repository

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use reqwest::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use crate::plugin_manager::pact_plugin_dir;
use crate::plugin_models::PactPluginManifest;

pub const DEFAULT_INDEX: &str = include_str!("../repository.index");
pub const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Struct representing the plugin repository index file
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginRepositoryIndex {
  /// Version of this index file
  pub index_version: usize,

  /// File format version of the index file
  pub format_version: usize,

  /// Timestamp (in UTC) that the file was created/updated
  pub timestamp: DateTime<Utc>,

  /// Plugin entries
  pub entries: HashMap<String, PluginEntry>
}

impl PluginRepositoryIndex {
  /// Looks up the plugin in the index. If no version is provided, will return the latest version
  pub fn lookup_plugin_version(&self, name: &str, version: &Option<String>) -> Option<PluginVersion> {
    self.entries.get(name).map(|entry| {
      let version = if let Some(version) = version {
        debug!("Installing plugin {}/{} from index", name, version);
        version.as_str()
      } else {
        debug!("Installing plugin {}/latest from index", name);
        entry.latest_version.as_str()
      };
      entry.versions.iter()
        .find(|v| v.version == version)
    })
      .flatten()
      .map(|entry| entry.clone())
  }
}

/// Struct to store the plugin version entries
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginEntry {
  /// Name of the plugin
  pub name: String,
  /// Latest version
  pub latest_version: String,
  /// All the plugin versions
  pub versions: Vec<PluginVersion>
}

/// Struct to store the plugin versions
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginVersion {
  /// Version of the plugin
  pub version: String,
  /// Source the manifest was loaded from
  pub source: ManifestSource,
  /// Manifest
  pub manifest: Option<PactPluginManifest>
}

/// Source that the plugin is loaded from
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum ManifestSource {
  /// Loaded from a file
  File(String),

  /// Loaded from a GitHub release
  GitHubRelease(String)
}

impl ManifestSource {
  /// Returns the name of the plugin source
  pub fn name(&self) -> String {
    match self {
      ManifestSource::File(_) => "file".to_string(),
      ManifestSource::GitHubRelease(_) => "GitHub release".to_string()
    }
  }

  /// Returns the associated value for the plugin source. For example, for a file source, returns
  /// the file path.
  pub fn value(&self) -> String {
    match self {
      ManifestSource::File(v) => v.clone(),
      ManifestSource::GitHubRelease(v) => v.clone()
    }
  }
}

impl PluginEntry {
  /// Create a new plugin entry from the provided manifest and source
  pub fn new(manifest: &PactPluginManifest, source: &ManifestSource) -> PluginEntry {
    PluginEntry {
      name: manifest.name.clone(),
      latest_version: manifest.version.clone(),
      versions: vec![PluginVersion {
        version: manifest.version.clone(),
        source: source.clone(),
        manifest: Some(manifest.clone())
      }]
    }
  }

  /// Adds the data from the plugin manifest as a version to the index
  pub fn add_version(&mut self, manifest: &PactPluginManifest, source: &ManifestSource) {
    if let Some(version) = self.versions.iter_mut()
      .find(|m| m.version == manifest.version) {
      version.source = source.clone();
      version.manifest = Some(manifest.clone());
    } else {
      self.versions.push(PluginVersion {
        version: manifest.version.clone(),
        source: source.clone(),
        manifest: Some(manifest.clone())
      });
    }
    self.update_latest_version();
  }

  fn update_latest_version(&mut self) {
    let latest_version = self.versions.iter()
      .max_by(|m1, m2| {
        let a = Version::parse(&m1.version).unwrap_or_else(|_| Version::new(0, 0, 0));
        let b = Version::parse(&m2.version).unwrap_or_else(|_| Version::new(0, 0, 0));
        a.cmp(&b)
      })
      .map(|m| m.version.clone())
      .unwrap_or_default();
    self.latest_version = latest_version.clone();
  }
}

impl Default for PluginRepositoryIndex {
  fn default() -> Self {
    #[cfg(feature = "datetime")]
    {
      let timestamp = Utc::now();
      PluginRepositoryIndex {
        index_version: 0,
        format_version: 0,
        timestamp,
        entries: Default::default()
      }
    }
    #[cfg(not(feature = "datetime"))]
    {
      use std::time::{SystemTime, UNIX_EPOCH};
      let now = SystemTime::now().duration_since(UNIX_EPOCH)
        .expect("system time before Unix epoch");
      let naive = chrono::NaiveDateTime::from_timestamp_opt(now.as_secs() as i64, now.subsec_nanos())
        .unwrap();
      let timestamp = DateTime::from_utc(naive, Utc);
      PluginRepositoryIndex {
        index_version: 0,
        format_version: 0,
        timestamp,
        entries: Default::default()
      }
    }
  }
}

/// Retrieves the latest repository index, first from GitHub, and if not able to, then any locally
/// cached index, otherwise defaults to the version compiled into the library.
pub async fn fetch_repository_index(
  http_client: &Client,
  default_index: Option<&str>
) -> anyhow::Result<PluginRepositoryIndex> {
  fetch_index_from_github(http_client)
    .await
    .or_else(|err| {
      warn!("Was not able to load index from GitHub - {}", err);
      load_local_index()
    })
    .or_else(|err| {
      warn!("Was not able to load local index, will use built in one - {}", err);
      toml::from_str::<PluginRepositoryIndex>(default_index.unwrap_or(DEFAULT_INDEX))
        .map_err(|err| anyhow!(err))
    })
}

fn load_local_index() -> anyhow::Result<PluginRepositoryIndex> {
  let plugin_dir = pact_plugin_dir()?;
  if !plugin_dir.exists() {
    return Err(anyhow!("Plugin directory does not exist"));
  }

  let repository_file = plugin_dir.join("repository.index");

  let sha = calculate_sha(&repository_file)?;
  let expected_sha = load_sha(&repository_file)?;
  if sha != expected_sha {
    return Err(anyhow!("Error: SHA256 digest does not match: expected {} but got {}", expected_sha, sha));
  }

  load_index_file(&repository_file)
}

async fn fetch_index_from_github(http_client: &Client) -> anyhow::Result<PluginRepositoryIndex> {
  info!("Fetching index from github");
  let index_contents = http_client.get("https://raw.githubusercontent.com/pact-foundation/pact-plugins/main/repository/repository.index")
    .send()
    .await?
    .text()
    .await?;

  let index_sha = http_client.get("https://raw.githubusercontent.com/pact-foundation/pact-plugins/main/repository/repository.index.sha256")
    .send()
    .await?
    .text()
    .await?;
  let mut hasher = Sha256::new();
  hasher.update(index_contents.as_bytes());
  let result = hasher.finalize();
  let calculated = format!("{:x}", result);

  if calculated != index_sha {
    return Err(anyhow!("Error: SHA256 digest from GitHub does not match: expected {} but got {}", index_sha, calculated));
  }

  if let Err(err) = cache_index(&index_contents, &index_sha) {
    warn!("Could not cache index to local file - {}", err);
  }

  Ok(toml::from_str(index_contents.as_str())?)
}

fn cache_index(index_contents: &String, sha: &String) -> anyhow::Result<()> {
  let plugin_dir = pact_plugin_dir()?;
  if !plugin_dir.exists() {
    fs::create_dir_all(&plugin_dir)?;
  }
  let repository_file = plugin_dir.join("repository.index");
  let mut f = File::create(repository_file)?;
  f.write_all(index_contents.as_bytes())?;
  let sha_file = plugin_dir.join("repository.index.sha256");
  let mut f2 = File::create(sha_file)?;
  f2.write_all(sha.as_bytes())?;
  Ok(())
}

/// Loads the index file from the given path
pub fn load_index_file(path: &PathBuf) -> anyhow::Result<PluginRepositoryIndex> {
  debug!(?path, "Loading index file");
  let f = File::open(path.as_path())?;
  let mut reader = BufReader::new(f);
  let mut buffer = String::new();
  reader.read_to_string(&mut buffer)?;
  let index: PluginRepositoryIndex = toml::from_str(buffer.as_str())?;
  Ok(index)
}

/// Returns the SHA file for a given repository file.
pub fn get_sha_file_for_repository_file(repository_file: &PathBuf) -> anyhow::Result<PathBuf> {
  let filename_base = repository_file.file_name()
    .ok_or_else(|| anyhow!("Could not get the filename for repository file '{}'", repository_file.to_string_lossy()))?
    .to_string_lossy();
  let sha_file = format!("{}.sha256", filename_base);
  let file = repository_file.parent()
    .ok_or_else(|| anyhow!("Could not get the parent path for repository file '{}'", repository_file.to_string_lossy()))?
    .join(sha_file.as_str());
  Ok(file)
}

/// Calculates the SHA hash for a given repository file path
pub fn calculate_sha(repository_file: &PathBuf) -> anyhow::Result<String> {
  let mut f = File::open(repository_file)?;
  let mut hasher = Sha256::new();
  let mut buffer = [0_u8; 256];
  let mut done = false;

  while !done {
    let amount = f.read(&mut buffer)?;
    if amount == 0 {
      done = true;
    } else if amount == 256 {
      hasher.update(&buffer);
    } else {
      let b = &buffer[0..amount];
      hasher.update(b);
    }
  }

  let result = hasher.finalize();
  let calculated = format!("{:x}", result);
  Ok(calculated)
}

/// Loads the SHA for a given repository file
pub fn load_sha(repository_file: &PathBuf) -> anyhow::Result<String> {
  let sha_file = get_sha_file_for_repository_file(repository_file)?;
  let mut f = File::open(sha_file)?;
  let mut buffer = String::new();
  f.read_to_string(&mut buffer)?;
  Ok(buffer)
}

#[cfg(test)]
mod tests {
  use std::time::{SystemTime, UNIX_EPOCH};

  use expectest::prelude::*;

  use crate::repository::PluginRepositoryIndex;

  #[test]
  fn plugin_repository_index_default() {
    let index = PluginRepositoryIndex::default();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    expect!(index.index_version).to(be_equal_to(0));
    expect!(index.format_version).to(be_equal_to(0));
    expect!(index.entries.len()).to(be_equal_to(0));

    let timestamp = index.timestamp.to_string();
    expect!(timestamp).to_not(be_equal_to("1970-01-01 00:00:00 UTC"));

    let ts = index.timestamp.naive_utc().timestamp() as u64;
    expect!(ts / 3600).to(be_equal_to(now / 3600));
  }
}
