use std::{env, fs};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context};
use itertools::Itertools;
use pact_plugin_driver::plugin_manager::load_plugin;
use pact_plugin_driver::plugin_models::PactPluginManifest;
use requestty::OnEsc;
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, info, trace};
use url::Url;
use pact_plugin_driver::download::{download_json_from_github, download_plugin_executable};
use pact_plugin_driver::repository::fetch_repository_index;

use crate::{find_plugin, resolve_plugin_dir};
use crate::repository::{APP_USER_AGENT, DEFAULT_INDEX};

use super::InstallationSource;

pub fn install_plugin(
  source: &String,
  _source_type: &Option<InstallationSource>,
  override_prompt: bool,
  skip_if_installed: bool,
  version: &Option<String>,
  skip_load: bool,
) -> anyhow::Result<()> {
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()?;
  let result = runtime.block_on(async {
    let http_client = reqwest::ClientBuilder::new()
      .user_agent(APP_USER_AGENT)
      .build()?;

    let install_url = Url::parse(source.as_str());
    if let Ok(install_url) = install_url {
      install_plugin_from_url(&http_client, install_url.as_str(), override_prompt, skip_if_installed,skip_load).await
    } else {
      install_known_plugin(&http_client, source.as_str(), override_prompt, skip_if_installed, version, skip_load).await
    }
  });

  trace!("Result = {:?}", result);
  runtime.shutdown_background();
  result
}

async fn install_known_plugin(
  http_client: &Client,
  name: &str,
  override_prompt: bool,
  skip_if_installed: bool,
  version: &Option<String>,
  skip_load: bool,
) -> anyhow::Result<()> {
  let index = fetch_repository_index(&http_client, Some(DEFAULT_INDEX)).await?;
  if let Some(entry) = index.entries.get(name) {
    let version = if let Some(version) = version {
      debug!("Installing plugin {}/{} from index", name, version);
      version.as_str()
    } else {
      debug!("Installing plugin {}/latest from index", name);
      entry.latest_version.as_str()
    };
    if let Some(version_entry) = entry.versions.iter().find(|v| v.version == version) {
      install_plugin_from_url(&http_client, version_entry.source.value().as_str(), override_prompt, skip_if_installed,skip_load).await
    } else {
      Err(anyhow!("'{}' is not a valid version for plugin '{}'", version, name))
    }
  } else {
    Err(anyhow!("'{}' is not a known plugin. Known plugins are: {}", name, index.entries.keys().join(", ")))
  }
}

async fn install_plugin_from_url(
  http_client: &Client,
  source_url: &str,
  override_prompt: bool,
  skip_if_installed: bool,
  skip_load: bool
) -> anyhow::Result<()> {
  let response = fetch_json_from_url(source_url, &http_client).await?;
  if let Some(map) = response.as_object() {
    if let Some(tag) = map.get("tag_name") {
      let tag = json_to_string(tag);
      debug!(%tag, "Found tag");
      let url = if source_url.ends_with("/latest") {
        source_url.strip_suffix("/latest").unwrap_or(source_url)
      } else {
        let suffix = format!("/tag/{}", tag);
        source_url.strip_suffix(suffix.as_str()).unwrap_or(source_url)
      };
      let manifest_json = download_json_from_github(&http_client, url, &tag, "pact-plugin.json")
        .await.context("Downloading manifest file from GitHub")?;
      let manifest: PactPluginManifest = serde_json::from_value(manifest_json)
        .context("Parsing JSON manifest file from GitHub")?;
      debug!(?manifest, "Loaded manifest from GitHub");

      if !skip_if_installed || !already_installed(&manifest) {
        println!("Installing plugin {} version {}", manifest.name, manifest.version);
        let plugin_dir = create_plugin_dir(&manifest, override_prompt)
          .context("Creating plugins directory")?;
        download_plugin_executable(&manifest, &plugin_dir, &http_client, url, &tag, true).await?;

        env::set_var("pact_do_not_track", "true");
        if !skip_load {
            load_plugin(&manifest.as_dependency())
          .await
          .and_then(|plugin| {
              println!("Installed plugin {} version {} OK", manifest.name, manifest.version);
              plugin.kill();
              Ok(())
          }) }
          else {
            return Ok(())
          }
      } else {
        println!("Skipping installing plugin {} version {} as it is already installed", manifest.name, manifest.version);
        Ok(())
      }
    } else {
      bail!("GitHub release page does not have a valid tag_name attribute");
    }
  } else {
    bail!("Response from source is not a valid JSON from a GitHub release page")
  }
}

pub(crate) async fn fetch_json_from_url(source: &str, http_client: &Client) -> anyhow::Result<Value> {
  info!(%source, "Fetching root document for source");
  let response: Value = http_client.get(source)
    .header("accept", "application/json")
    .send()
    .await.context("Fetching root document for source")?
    .json()
    .await.context("Parsing root JSON document for source")?;
  debug!(?response, "Got response");
  Ok(response)
}

fn already_installed(manifest: &PactPluginManifest) -> bool {
  if let Ok(res) = find_plugin(&manifest.name, &Some(manifest.version.clone())) {
    return res.len() > 0
  }

  return false
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

pub(crate) fn json_to_string(value: &Value) -> String {
  match value {
    Value::String(s) => s.clone(),
    _ => value.to_string()
  }
}
