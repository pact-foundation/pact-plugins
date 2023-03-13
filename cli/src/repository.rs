use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context};
use chrono::{DateTime, Local, Utc};
use comfy_table::presets::UTF8_FULL;
use comfy_table::Table;
use itertools::Itertools;
use pact_plugin_driver::plugin_models::PactPluginManifest;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tracing::{debug, info, trace};

use crate::{PluginVersionCommand, RepositoryCommands};
use crate::install::{download_json_from_github, fetch_json_from_url, json_to_string};
use crate::repository::ManifestSource::GitHubRelease;

pub(crate) fn handle_command(repository_command: &RepositoryCommands) -> anyhow::Result<()> {
  match repository_command {
    RepositoryCommands::Validate { filename } => validate_repository_file(filename),
    RepositoryCommands::New { filename, overwrite } => new_repository(filename, *overwrite),
    RepositoryCommands::AddPluginVersion(command) => handle_add_plugin_command(command),
    RepositoryCommands::AddAllPluginVersions { repository_file, base_url, owner, repository } => handle_add_all_versions(repository_file, base_url, owner, repository),
    RepositoryCommands::YankVersion => { todo!() }
    RepositoryCommands::List { filename } => list_entries(filename),
    RepositoryCommands::ListVersions { filename, name } => list_versions(filename, name)
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
#[derive(Serialize, Deserialize, Debug, Clone)]
struct PluginEntry {
  /// Name of the plugin
  name: String,
  /// Latest version
  latest_version: String,
  /// All the plugin versions
  versions: Vec<PluginVersion>
}

/// Struct to store the plugin versions
#[derive(Serialize, Deserialize, Debug, Clone)]
struct PluginVersion {
  /// Version of the plugin
  version: String,
  /// Source the manifest was loaded from
  source: ManifestSource,
  /// Manifest
  manifest: Option<PactPluginManifest>
}

/// Struct to store the plugin versions
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
enum ManifestSource {
  /// Loaded from a file
  File(String),

  /// Loaded from a GitHub release
  GitHubRelease(String)
}

impl ManifestSource {
  fn name(&self) -> String {
    match self {
      ManifestSource::File(_) => "file".to_string(),
      ManifestSource::GitHubRelease(_) => "GitHub release".to_string()
    }
  }

  fn value(&self) -> String {
    match self {
      ManifestSource::File(v) => v.clone(),
      ManifestSource::GitHubRelease(v) => v.clone()
    }
  }
}

impl PluginEntry {
  fn new(manifest: &PactPluginManifest, source: &ManifestSource) -> PluginEntry {
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

  fn add_version(&mut self, manifest: &PactPluginManifest, source: &ManifestSource) {
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
    generate_sha_digest(&path)?;

    println!("Created new blank repository file '{}'", abs_path.to_string_lossy());

    Ok(())
  }
}

fn validate_repository_file(filename: &String) -> anyhow::Result<()> {
  let path = PathBuf::from(filename.as_str());
  let abs_path = path.canonicalize().unwrap_or(path.clone());
  if path.exists() {
    let sha = calculate_sha(&path)?;
    let expected_sha = load_sha(&path)?;
    if sha != expected_sha {
      return Err(anyhow!("Error: SHA256 digest does not match: expected {} but got {}", expected_sha, sha));
    }

    let index = load_index_file(&path)?;

    if index.format_version != 0 {
      return Err(anyhow!("Error: format_version is not valid: {}", index.format_version));
    }

    println!("'{}' OK", abs_path.to_string_lossy());
    println!();

    let mut table = Table::new();
    table
      .load_preset(UTF8_FULL)
      .set_header(vec!["Key", "Value", ""]);

    table.add_row(vec![ "Format Version", index.format_version.to_string().as_str(), "" ]);
    table.add_row(vec![ "Index Version", index.index_version.to_string().as_str(), "" ]);

    let local_timestamp: DateTime<Local> = index.timestamp.into();
    let additional = format!("Local: {}", local_timestamp);
    table.add_row(vec![ "Last Modified", index.timestamp.to_string().as_str(), additional.as_str() ]);

    table.add_row(vec![ "Plugin Entries", index.entries.len().to_string().as_str(), "" ]);
    let versions = index.entries.iter()
      .fold(0, |acc, (_, entry)| acc + entry.versions.len());
    table.add_row(vec![ "Total Versions", versions.to_string().as_str(), "" ]);
    table.add_row(vec![ "SHA", sha.as_str(), "" ]);

    println!("{table}");

    Ok(())
  } else {
    Err(anyhow!("Repository file '{}' does not exist", abs_path.to_string_lossy()))
  }
}

fn load_index_file(path: &PathBuf) -> anyhow::Result<PluginRepositoryIndex> {
  let f = File::open(path.clone())?;
  let mut reader = BufReader::new(f);
  let mut buffer = String::new();
  reader.read_to_string(&mut buffer)?;
  let index: PluginRepositoryIndex = toml::from_str(buffer.as_str())?;
  Ok(index)
}

fn handle_add_plugin_command(command: &PluginVersionCommand) -> anyhow::Result<()> {
  match command {
    PluginVersionCommand::File { repository_file, file } => {
      let repository_file = validate_filename(repository_file, "Repository")?;
      let file = validate_filename(file, "Plugin manifest file")?;
      let f = File::open(&file)?;
      let reader = BufReader::new(f);
      let manifest: PactPluginManifest = serde_json::from_reader(reader)?;
      let mut index = load_index_file(&repository_file)?;
      add_manifest_to_index(&repository_file, &manifest, &mut index, &ManifestSource::File(file.to_string_lossy().to_string()))?;
      Ok(())
    }
    PluginVersionCommand::GitHub { repository_file, url } => {
      let repository_file = validate_filename(repository_file, "Repository")?;
      let mut index = load_index_file(&repository_file)?;
      let manifest = download_manifest_from_github(url)
        .context("Downloading manifest file from GitHub")?;
      add_manifest_to_index(&repository_file, &manifest, &mut index, &ManifestSource::GitHubRelease(url.to_string()))?;
      Ok(())
    }
  }
}

fn add_manifest_to_index(
  repository_file: &PathBuf,
  manifest: &PactPluginManifest,
  index: &mut PluginRepositoryIndex,
  source: &ManifestSource
) -> anyhow::Result<()> {
  index.entries
    .entry(manifest.name.clone())
    .and_modify(|entry| entry.add_version(&manifest, source))
    .or_insert_with(|| PluginEntry::new(&manifest, source));
  index.index_version += 1;
  let toml = toml::to_string(&index)?;
  let mut f = File::create(&repository_file)?;
  f.write_all(toml.as_bytes())?;
  generate_sha_digest(&repository_file)?;

  println!("Added plugin version {}/{} to repository file '{}'",
           manifest.name, manifest.version, repository_file.to_string_lossy());
  Ok(())
}

fn download_manifest_from_github(source: &String) -> anyhow::Result<PactPluginManifest> {
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()?;

  let result = runtime.block_on(async {
    let http_client = reqwest::ClientBuilder::new()
      .build()?;

    let response = fetch_json_from_url(source, &http_client).await?;
    if let Some(map) = response.as_object() {
      if let Some(tag) = map.get("tag_name") {
        let tag = json_to_string(tag);
        debug!(%tag, "Found tag");
        let url = if source.ends_with("/latest") {
          source.strip_suffix("/latest").unwrap_or(source)
        } else {
          let suffix = format!("/tag/{}", tag);
          source.strip_suffix(suffix.as_str()).unwrap_or(source)
        };
        let manifest_json = download_json_from_github(&http_client, url, &tag, "pact-plugin.json")
          .await.context("Downloading manifest file from GitHub")?;
        let manifest: PactPluginManifest = serde_json::from_value(manifest_json)
          .context("Parsing JSON manifest file from GitHub")?;
        debug!(?manifest, "Loaded manifest from GitHub");
        Ok(manifest)
      } else {
        bail!("GitHub release page does not have a valid tag_name attribute");
      }
    } else {
      bail!("Response from source is not a valid JSON from a GitHub release page")
    }
  });
  trace!("Result = {:?}", result);
  runtime.shutdown_background();
  result
}

fn generate_sha_digest(repository_file: &PathBuf) -> anyhow::Result<String> {
  let file = get_sha_file_for_repository_file(repository_file)?;
  let calculated = calculate_sha(repository_file)?;

  let mut f = File::create(file)?;
  f.write_all(calculated.as_bytes())?;
  Ok(calculated)
}

fn get_sha_file_for_repository_file(repository_file: &PathBuf) -> anyhow::Result<PathBuf> {
  let filename_base = repository_file.file_name()
    .ok_or_else(|| anyhow!("Could not get the filename for repository file '{}'", repository_file.to_string_lossy()))?
    .to_string_lossy();
  let sha_file = format!("{}.sha256", filename_base);
  let file = repository_file.parent()
    .ok_or_else(|| anyhow!("Could not get the parent path for repository file '{}'", repository_file.to_string_lossy()))?
    .join(sha_file.as_str());
  Ok(file)
}

fn calculate_sha(repository_file: &PathBuf) -> anyhow::Result<String> {
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

fn load_sha(repository_file: &PathBuf) -> anyhow::Result<String> {
  let sha_file = get_sha_file_for_repository_file(repository_file)?;
  let mut f = File::open(sha_file)?;
  let mut buffer = String::new();
  f.read_to_string(&mut buffer)?;
  Ok(buffer)
}

fn validate_filename(filename: &str, file_description: &str) -> anyhow::Result<PathBuf> {
  let path = PathBuf::from(filename);
  let abs_path = path.canonicalize().unwrap_or(path.clone());
  if abs_path.exists() {
    Ok(abs_path)
  } else {
    Err(anyhow!("{} file '{}' does not exist", file_description, abs_path.to_string_lossy()))
  }
}

fn list_versions(filename: &String, plugin_name: &String) -> anyhow::Result<()> {
  let repository_file = validate_filename(filename, "Repository")?;
  let index = load_index_file(&repository_file)?;
  let mut table = Table::new();
  table
    .load_preset(UTF8_FULL)
    .set_header(vec!["Name", "Version", "Source", "Value"]);

  if let Some(entry) = index.entries.get(plugin_name.as_str()) {
    for version in entry.versions.iter().sorted_by_key(|v| v.version.clone()) {
      table.add_row(vec![
        plugin_name.as_str(),
        version.version.as_str(),
        version.source.name().as_str(),
        version.source.value().as_str()
      ]);
    }
  }

  println!("{table}");

  Ok(())
}

fn list_entries(filename: &String) -> anyhow::Result<()> {
  let repository_file = validate_filename(filename, "Repository")?;
  let index = load_index_file(&repository_file)?;

  let mut table = Table::new();
  table
    .load_preset(UTF8_FULL)
    .set_header(vec!["Key", "Name", "Latest Version", "Versions"]);

  for (key, entry) in index.entries.iter().sorted_by_key(|(k, _)| k.clone()) {
    table.add_row(vec![
      key.as_str(),
      entry.name.as_str(),
      entry.latest_version.as_str(),
      entry.versions.len().to_string().as_str()
    ]);
  }

  println!("{table}");

  Ok(())
}

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

fn handle_add_all_versions(
  repository_file: &String,
  base_url: &Option<String>,
  owner: &String,
  repository: &String
) -> anyhow::Result<()> {
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()?;
  let repository_file = validate_filename(repository_file, "Repository")?;
  let mut index = load_index_file(&repository_file)?;
  let source = format!("{}/{}/{}/releases", base_url.clone().unwrap_or("https://api.github.com/repos".to_string()),
    owner, repository);

  let result = runtime.block_on(async move {
    let http_client = reqwest::ClientBuilder::new()
      .user_agent(APP_USER_AGENT)
      .build()?;

    let response = fetch_json_from_url(source.as_str(), &http_client).await?;
    match response {
      Value::Array(values) => {
        for value in values {
          match value {
            Value::Object(attributes) => if let Some(url) = attributes.get("html_url") {
              let release_url = json_to_string(url);
              let source = GitHubRelease(release_url);
              if let Some(assets) = attributes.get("assets") {
                if let Value::Array(assets) = assets {
                  if let Some(manifest_url) = assets.iter().find_map(|item| {
                    match item {
                      Value::Object(attributes) => if let Some(name) = attributes.get("name") {
                        let name = json_to_string(name);
                        if name == "pact-plugin.json" {
                          if let Some(url) = attributes.get("browser_download_url") {
                            Some(json_to_string(url))
                          } else {
                            None
                          }
                        } else {
                          None
                        }
                      } else {
                        None
                      }
                      _ => None
                    }
                  }) {
                    let manifest_json = http_client.get(manifest_url).send().await?
                      .json().await?;
                    let manifest: PactPluginManifest = serde_json::from_value(manifest_json)
                      .context("Parsing JSON manifest file from GitHub")?;
                    index.entries
                      .entry(manifest.name.clone())
                      .and_modify(|entry| entry.add_version(&manifest, &source))
                      .or_insert_with(|| PluginEntry::new(&manifest, &source));
                  }
                }
              }
            }
            _ => bail!("Was expecting a JSON Object, but got '{}'", value)
          }
        }

        index.index_version += 1;
        let toml = toml::to_string(&index)?;
        let mut f = File::create(&repository_file)?;
        f.write_all(toml.as_bytes())?;
        generate_sha_digest(&repository_file)?;

        println!("Added all plugin versions from {}/{} to repository file '{}'",
                 owner, repository, repository_file.to_string_lossy());
        Ok(())
      }
      _ => bail!("Was expecting a JSON Array, but got '{}'", response)
    }
  });

  trace!("Result = {:?}", result);
  runtime.shutdown_background();
  result
}
