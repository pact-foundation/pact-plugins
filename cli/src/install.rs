use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use anyhow::{bail, Error};
use pact_plugin_driver::plugin_models::PactPluginManifest;
use requestty::OnEsc;
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, info};
use crate::resolve_plugin_dir;

use super::InstallationSource;

pub fn install_plugin(
  source: &String,
  _source_type: &Option<InstallationSource>,
  override_prompt: bool
) -> anyhow::Result<()> {
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()?;

  runtime.block_on(async {
    let http_client = reqwest::ClientBuilder::new()
      .build()?;

    info!(%source, "Fetching root document for source");
    let response: Value = http_client.get(source)
      .header("accept", "application/json")
      .send()
      .await?
      .json()
      .await?;

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
        let manifest_json = download_json_from_github(&http_client, url, &tag, "pact-plugin.json").await?;
        let manifest: PactPluginManifest = serde_json::from_value(manifest_json)?;
        debug!(?manifest, "Loaded manifest from GitHub");

        println!("Installing plugin {} version {}", manifest.name, manifest.version);
        let plugin_dir = create_plugin_dir(&manifest, override_prompt)?;
        download_plugin_executable(&manifest, &plugin_dir, override_prompt)
      } else {
        bail!("GitHub release page does not have a valid tag_name attribute");
      }
    } else {
      bail!("Response from source is not a valid JSON from a GitHub release page")
    }
  })
}

fn download_plugin_executable(manifest: &PactPluginManifest, plugin_dir: &PathBuf, override_prompt: bool) -> anyhow::Result<()> {
  // Check for a single exec .gz file

}

fn create_plugin_dir(manifest: &PactPluginManifest, override_prompt: bool) -> anyhow::Result<PathBuf> {
  let (_, dir) = resolve_plugin_dir();
  let plugins_dir = PathBuf::from(dir);
  if !plugins_dir.exists() {
    info!(plugins_dir = %plugins_dir.display(), "Creating plugins directory");
    fs::create_dir_all(plugins_dir.clone())?;
  }

  let plugin_dir = plugins_dir.join(format!("{}-{}", manifest.name, manifest.version));
  if plugin_dir.exists() {
    if !override_prompt && !prompt_continue(manifest) {
      println!("Plugin already exists, aborting.");
      std::process::exit(1);
    } else {
      info!("Deleting contents of plugin directory");
      fs::remove_dir_all(plugin_dir.clone())?;
      fs::create_dir(plugin_dir.clone())?;
    }
  } else {
    info!(plugin_dir = %plugin_dir.display(), "Creating plugin directory");
    fs::create_dir(plugin_dir.clone())?;
  }

  info!("Writing plugin manifest file");
  let file_name = plugin_dir.join("pact-plugin.json");
  let mut f = File::create(file_name)?;
  let json = serde_json::to_string(manifest)?;
  f.write_all(json.as_bytes())?;

  Ok(plugin_dir.clone())
}

fn prompt_continue(manifest: &PactPluginManifest) -> bool {
  let question = requestty::Question::confirm("overwrite_plugin")
    .message(format!("Plugin with name '{}' and version '{}' already exists. Overwrite it?", manifest.name, manifest.version))
    .default(false)
    .on_esc(OnEsc::Terminate)
    .build();
  if let Ok(result) = requestty::prompt_one(question) {
    if let Some(result) = result.as_bool() {
      result
    } else {
      false
    }
  } else {
    false
  }
}

fn json_to_string(value: &Value) -> String {
  match value {
    Value::String(s) => s.clone(),
    _ => value.to_string()
  }
}

async fn download_json_from_github(http_client: &Client, base_url: &str, tag: &String, filename: &str) -> anyhow::Result<Value> {
  let url = format!("{}/download/{}/{}", base_url, tag, filename);
  debug!("Downloading JSON file from {}", url);
  Ok(http_client.get(url)
    .send()
    .await?
    .json()
    .await?)
}
