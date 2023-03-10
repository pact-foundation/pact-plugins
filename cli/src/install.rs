use std::cmp::min;
use std::{env, fs, io};
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use anyhow::{anyhow, bail, Context};
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use os_info::Type;
use pact_plugin_driver::plugin_manager::load_plugin;
use pact_plugin_driver::plugin_models::PactPluginManifest;
use requestty::OnEsc;
use reqwest::Client;
use serde_json::Value;
use sha2::{Sha256, Digest};
use tracing::{debug, info, trace};

use crate::{find_plugin, resolve_plugin_dir};

use super::InstallationSource;

pub fn install_plugin(
  source: &String,
  _source_type: &Option<InstallationSource>,
  override_prompt: bool,
  skip_if_installed: bool
) -> anyhow::Result<()> {
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

        if !skip_if_installed || !already_installed(&manifest) {
          println!("Installing plugin {} version {}", manifest.name, manifest.version);
          let plugin_dir = create_plugin_dir(&manifest, override_prompt)
            .context("Creating plugins directory")?;
          download_plugin_executable(&manifest, &plugin_dir, &http_client, url, &tag).await?;

          env::set_var("pact_do_not_track", "true");
          load_plugin(&manifest.as_dependency())
            .await
            .and_then(|plugin| {
              println!("Installed plugin {} version {} OK", manifest.name, manifest.version);
              plugin.kill();
              Ok(())
            })
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
  });
  trace!("Result = {:?}", result);
  runtime.shutdown_background();
  result
}

pub(crate) async fn fetch_json_from_url(source: &str, http_client: &Client) -> anyhow::Result<Value> {
  info!(%source, "Fetching root document for source");
  let response: Value = http_client.get(source)
    .header("accept", "application/json")
    .send()
    .await.context("Fetching root document for source")?
    .json()
    .await.context("Parsing root JSON document for source")?;
  Ok(response)
}

fn already_installed(manifest: &PactPluginManifest) -> bool {
  find_plugin(&manifest.name, &Some(manifest.version.clone())).is_ok()
}

async fn download_plugin_executable(
  manifest: &PactPluginManifest,
  plugin_dir: &PathBuf,
  http_client: &Client,
  base_url: &str,
  tag: &String
) -> anyhow::Result<PathBuf> {
  let (os, arch) = os_and_arch()?;

  // Check for a single exec .gz file
  let ext = if os == "windows" { ".exe" } else { "" };
  let gz_file = format!("pact-{}-plugin-{}-{}{}.gz", manifest.name, os, arch, ext);
  let sha_file = format!("pact-{}-plugin-{}-{}{}.gz.sha256", manifest.name, os, arch, ext);
  if github_file_exists(http_client, base_url, tag, gz_file.as_str()).await? {
    debug!(file = %gz_file, "Found a GZipped file");
    let file = download_file_from_github(http_client, base_url, tag, gz_file.as_str(), plugin_dir).await?;

    if github_file_exists(http_client, base_url, tag, sha_file.as_str()).await? {
      let sha_file = download_file_from_github(http_client, base_url, tag, sha_file.as_str(), plugin_dir).await?;
      check_sha(&file, &sha_file)?;
      fs::remove_file(sha_file)?;
    }

    let file = gunzip_file(&file, plugin_dir, manifest, ext)?;
    #[cfg(unix)]
    {
      let mut perms = fs::metadata(&file)?.permissions();
      perms.set_mode(0o775);
      fs::set_permissions(&file, perms)?;
    }

    return Ok(file);
  }

  // Check for a arch specific Zip file
  let zip_file = format!("pact-{}-plugin-{}-{}.zip", manifest.name, os, arch);
  let zip_sha_file = format!("pact-{}-plugin-{}-{}.zip.sha256", manifest.name, os, arch);
  if github_file_exists(http_client, base_url, tag, zip_file.as_str()).await? {
    return download_zip_file(plugin_dir, http_client, base_url, tag, zip_file, zip_sha_file).await;
  }

  // Check for a Zip file
  let zip_file = format!("pact-{}-plugin.zip", manifest.name);
  let zip_sha_file = format!("pact-{}-plugin.zip.sha256", manifest.name);
  if github_file_exists(http_client, base_url, tag, zip_file.as_str()).await? {
    return download_zip_file(plugin_dir, http_client, base_url, tag, zip_file, zip_sha_file).await;
  }

  bail!("Did not find a matching file pattern on GitHub to install")
}

async fn download_zip_file(plugin_dir: &PathBuf, http_client: &Client, base_url: &str, tag: &String, zip_file: String, zip_sha_file: String) -> anyhow::Result<PathBuf> {
  debug!(file = %zip_file, "Found a Zip file");
  let file = download_file_from_github(http_client, base_url, tag, zip_file.as_str(), plugin_dir).await?;

  if github_file_exists(http_client, base_url, tag, zip_sha_file.as_str()).await? {
    let sha_file = download_file_from_github(http_client, base_url, tag, zip_sha_file.as_str(), plugin_dir).await?;
    check_sha(&file, &sha_file)?;
    fs::remove_file(sha_file)?;
  }

  unzip_file(&file, plugin_dir)
}

fn unzip_file(zip_file: &PathBuf, plugin_dir: &PathBuf) -> anyhow::Result<PathBuf> {
  let mut archive = zip::ZipArchive::new(File::open(zip_file)?)?;

  for i in 0..archive.len() {
    let mut file = archive.by_index(i).unwrap();
    let outpath = match file.enclosed_name() {
      Some(path) => plugin_dir.join(path),
      None => continue
    };

    if (*file.name()).ends_with('/') {
      debug!("Dir {} extracted to \"{}\"", i, outpath.display());
      fs::create_dir_all(&outpath)?;
    } else {
      debug!("File {} extracted to \"{}\" ({} bytes)", i, outpath.display(), file.size());
      if let Some(p) = outpath.parent() {
        if !p.exists() {
          fs::create_dir_all(&p)?;
        }
      }
      let mut outfile = File::create(&outpath)?;
      io::copy(&mut file, &mut outfile)?;
    }

    #[cfg(unix)]
    {
      if let Some(mode) = file.unix_mode() {
        fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
      }
    }
  }

  Ok(plugin_dir.clone())
}

fn gunzip_file(
  gz_file: &PathBuf,
  plugin_dir: &PathBuf,
  manifest: &PactPluginManifest,
  ext: &str
) -> anyhow::Result<PathBuf> {
  let file = if ext.is_empty() {
    plugin_dir.join(&manifest.entry_point)
  } else {
    plugin_dir.join(&manifest.entry_point)
      .with_extension(ext.strip_prefix('.').unwrap_or(ext))
  };
  let mut f = File::create(file.clone())?;
  let mut gz = GzDecoder::new(File::open(gz_file)?);

  let bytes = io::copy(&mut gz, &mut f)?;
  debug!(file = %file.display(), "Wrote {} bytes", bytes);
  fs::remove_file(gz_file)?;

  Ok(file)
}

fn check_sha(file: &PathBuf, sha_file: &PathBuf) -> anyhow::Result<()> {
  debug!(file = %file.display(), sha_file = %sha_file.display(), "Checking SHA of downloaded file");
  let sha = fs::read_to_string(sha_file).context("Could not read SHA file")?;
  let sha = sha.split(' ').next().ok_or(anyhow!("SHA file is not correctly formatted"))?;
  debug!("Downloaded SHA {}", sha);

  let mut hasher = Sha256::new();
  let mut f = File::open(file.clone())?;
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
  debug!("Calculated SHA {}", calculated);
  if calculated == sha {
    Ok(())
  } else {
    Err(anyhow!("Downloaded file {} has a checksum mismatch: {} != {}",
      file.display(), sha, calculated))
  }
}

async fn download_file_from_github(
  http_client: &Client,
  base_url: &str,
  tag: &String,
  filename: &str,
  plugin_dir: &PathBuf
) -> anyhow::Result<PathBuf> {
  let url = format!("{}/download/{}/{}", base_url, tag, filename);
  debug!("Downloading file from {}", url);

  let res = http_client.get(url.as_str()).send().await?;
  let total_size = res.content_length()
    .ok_or(anyhow!("Failed to get content length from '{}'", url))?;

  let pb = ProgressBar::new(total_size);
  pb.set_style(
    ProgressStyle::with_template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
    .unwrap()
    .progress_chars("#>-"));
  pb.set_message(format!("Downloading {}", url));

  let path = plugin_dir.join(filename);
  let mut file = File::create(path.clone())?;
  let mut downloaded: u64 = 0;
  let mut stream = res.bytes_stream();

  while let Some(item) = stream.next().await {
    let chunk = item?;
    file.write_all(&chunk)?;
    let new = min(downloaded + (chunk.len() as u64), total_size);
    downloaded = new;
    pb.set_position(new);
  }

  pb.finish_with_message(format!("Downloaded {} to {}", url, path.display()));
  Ok(path.clone())
}

async fn github_file_exists(http_client: &Client, base_url: &str, tag: &String, filename: &str) -> anyhow::Result<bool> {
  let url = format!("{}/download/{}/{}", base_url, tag, filename);
  debug!("Checking existence of file from {}", url);
  Ok(http_client.head(url)
    .send()
    .await?
    .status().is_success())
}

fn os_and_arch() -> anyhow::Result<(&'static str, &'static str)> {
  let os_info = os_info::get();
  debug!("Detected OS: {}", os_info);

  let os = match os_info.os_type() {
    Type::Alpine | Type::Amazon| Type::Android| Type::Arch| Type::CentOS| Type::Debian |
    Type::EndeavourOS | Type::Fedora | Type::Gentoo | Type::Linux | Type::Manjaro | Type::Mariner |
    Type::Mint | Type::NixOS | Type::openSUSE | Type::OracleLinux | Type::Redhat |
    Type::RedHatEnterprise | Type::Pop | Type::Raspbian | Type::Solus | Type::SUSE |
    Type::Ubuntu => "linux",
    Type::Macos => "osx",
    Type::Windows => "windows",
    _ => bail!("{} is not a supported operating system", os_info)
  };

  Ok((os, std::env::consts::ARCH))
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

pub(crate) async fn download_json_from_github(http_client: &Client, base_url: &str, tag: &String, filename: &str) -> anyhow::Result<Value> {
  let url = format!("{}/download/{}/{}", base_url, tag, filename);
  debug!("Downloading JSON file from {}", url);
  Ok(http_client.get(url)
    .send()
    .await?
    .json()
    .await?)
}
