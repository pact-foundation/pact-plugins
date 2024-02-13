//! Module that provides functions for downloading plugin files

use std::{fs, io};
use std::cmp::min;
use std::fs::File;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use futures_util::StreamExt;

use crate::plugin_models::PactPluginManifest;
use crate::utils::os_and_arch;
use tar::Archive;

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

/// Downloads a JSON file from a GitHub release URL
pub async fn download_json_from_github(
  http_client: &Client,
  base_url: &str,
  tag: &String,
  filename: &str
) -> anyhow::Result<Value> {
  let url = format!("{}/download/{}/{}", base_url, tag, filename);
  debug!("Downloading JSON file from {}", url);
  Ok(http_client.get(url)
    .send()
    .await?
    .json()
    .await?)
}

/// Downloads the plugin executable from the given base URL for the current OS and architecture.
pub async fn download_plugin_executable(
  manifest: &PactPluginManifest,
  plugin_dir: &PathBuf,
  http_client: &Client,
  base_url: &str,
  tag: &String,
  display_progress: bool
) -> anyhow::Result<PathBuf> {
  let (os, arch, musl) = os_and_arch()?;

  // Check for a single exec .gz file
    let ext = if os == "windows" { ".exe" } else { "" };
    let mut gz_file = format!("pact-{}-plugin-{}-{}{}.gz", manifest.name, os, arch, ext);
    let mut sha_file = format!("pact-{}-plugin-{}-{}{}.gz.sha256", manifest.name, os, arch, ext);
    if musl != "" {
      let gz_file_musl = format!("pact-{}-plugin-{}-{}{}{}.gz", manifest.name, os, arch, musl, ext);
      let sha_file_musl = format!("pact-{}-plugin-{}-{}{}{}.gz.sha256", manifest.name, os, arch, musl, ext);
      if github_file_exists(http_client, base_url, tag, gz_file_musl.as_str()).await? {
        gz_file = gz_file_musl;
        sha_file = sha_file_musl;
      } else {
        warn!("musl detected, but no musl specific plugin implementation found - you may experience issues");
      }
    }

    if github_file_exists(http_client, base_url, tag, gz_file.as_str()).await? {
    debug!(file = %gz_file, "Found a GZipped file");
    let file = download_file_from_github(http_client, base_url, tag, gz_file.as_str(), plugin_dir, display_progress).await?;

    if github_file_exists(http_client, base_url, tag, sha_file.as_str()).await? {
      let sha_file = download_file_from_github(http_client, base_url, tag, sha_file.as_str(), plugin_dir, display_progress).await?;
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

  // Check for an arch specific Zip file
  let mut zip_file = format!("pact-{}-plugin-{}-{}.zip", manifest.name, os, arch);
  let mut zip_sha_file = format!("pact-{}-plugin-{}-{}.zip.sha256", manifest.name, os, arch);

  if musl != "" {
    let zip_file_musl = format!("pact-{}-plugin-{}-{}{}.zip", manifest.name, os, arch, musl);
    let zip_sha_file_musl = format!("pact-{}-plugin-{}-{}{}.zip.sha256", manifest.name, os, arch, musl);
    if github_file_exists(http_client, base_url, tag, zip_file_musl.as_str()).await? {
      zip_file = zip_file_musl;
      zip_sha_file = zip_sha_file_musl;
    } else {
      warn!("musl detected, but no musl specific plugin implementation found - you may experience issues");
    }
  }

  if github_file_exists(http_client, base_url, tag, zip_file.as_str()).await? {
    return download_zip_file(plugin_dir, http_client, base_url, tag, zip_file, zip_sha_file, display_progress).await;
  }

  // Check for a Zip file
  let zip_file = format!("pact-{}-plugin.zip", manifest.name);
  let zip_sha_file = format!("pact-{}-plugin.zip.sha256", manifest.name);
  if github_file_exists(http_client, base_url, tag, zip_file.as_str()).await? {
    return download_zip_file(plugin_dir, http_client, base_url, tag, zip_file, zip_sha_file, display_progress).await;
  }

  // Check for a tar.gz file
  let tar_gz_file = format!("pact-{}-plugin.tar.gz", manifest.name);
  let tar_gz_sha_file = format!("pact-{}-plugin.tar.gz.sha256", manifest.name);
  if github_file_exists(http_client, base_url, tag, tar_gz_file.as_str()).await? {
    return download_tar_gz_file(plugin_dir, http_client, base_url, tag, tar_gz_file, tar_gz_sha_file, display_progress).await;
  }

  // Check for an arch specific tar.gz file
  let mut tar_gz_file = format!("pact-{}-plugin-{}-{}{}.tar.gz", manifest.name, os, arch, musl);
  let mut tar_gz_sha_file = format!("pact-{}-plugin-{}-{}{}.tar.gz.sha256", manifest.name, os, arch, musl);

  if musl != "" {
    let tar_gz_file_musl = format!("pact-{}-plugin-{}-{}{}.tar.gz", manifest.name, os, arch, musl);
    let tar_gz_sha_file_musl = format!("pact-{}-plugin-{}-{}{}.tar.gz.sha256", manifest.name, os, arch, musl);
    if github_file_exists(http_client, base_url, tag, tar_gz_file_musl.as_str()).await? {
      tar_gz_file = tar_gz_file_musl;
      tar_gz_sha_file = tar_gz_sha_file_musl;
    } else {
      warn!("musl detected, but no musl specific plugin implementation found - you may experience issues");
    }
  }

  if github_file_exists(http_client, base_url, tag, tar_gz_file.as_str()).await? {
    return download_tar_gz_file(plugin_dir, http_client, base_url, tag, tar_gz_file, tar_gz_sha_file, display_progress).await;
  }

  // Check for an arch specific tgz file
  let mut tgz_file = format!("pact-{}-plugin-{}-{}{}.tgz", manifest.name, os, arch, musl);
  let mut tgz_sha_file = format!("pact-{}-plugin-{}-{}{}.tgz.sha256", manifest.name, os, arch, musl);

  if musl != "" {
    let tgz_file_musl = format!("pact-{}-plugin-{}-{}{}.tgz", manifest.name, os, arch, musl);
    let tgz_sha_file_musl = format!("pact-{}-plugin-{}-{}{}.tgz.sha256", manifest.name, os, arch, musl);
    if github_file_exists(http_client, base_url, tag, tgz_file_musl.as_str()).await? {
      tgz_file = tgz_file_musl;
      tgz_sha_file = tgz_sha_file_musl;
    } else {
      warn!("musl detected, but no musl specific plugin implementation found - you may experience issues");
    }
  }

  if github_file_exists(http_client, base_url, tag, tgz_file.as_str()).await? {
    return download_tar_gz_file(plugin_dir, http_client, base_url, tag, tgz_file, tgz_sha_file, display_progress).await;
  }

  // Check for a tgz file
  let tgz_file = format!("pact-{}-plugin.tgz", manifest.name);
  let tgz_sha_file = format!("pact-{}-plugin.tgz.sha256", manifest.name);
  if github_file_exists(http_client, base_url, tag, tgz_file.as_str()).await? {
    return download_tar_gz_file(plugin_dir, http_client, base_url, tag, tgz_file, tgz_sha_file, display_progress).await;
  }

  bail!("Did not find a matching file pattern on GitHub to install")
}

async fn github_file_exists(http_client: &Client, base_url: &str, tag: &String, filename: &str) -> anyhow::Result<bool> {
  let url = format!("{}/download/{}/{}", base_url, tag, filename);
  debug!("Checking existence of file from {}", url);
  Ok(http_client.head(url)
    .send()
    .await?
    .status().is_success())
}

/// Downloads a plugin zip file from GitHub and installs it
pub async fn download_zip_file(
  plugin_dir: &PathBuf,
  http_client: &Client,
  base_url: &str,
  tag: &String,
  zip_file: String,
  zip_sha_file: String,
  display_progress: bool
) -> anyhow::Result<PathBuf> {
  debug!(file = %zip_file, "Found a Zip file");
  let file = download_file_from_github(http_client, base_url, tag, zip_file.as_str(), plugin_dir, display_progress).await?;

  if github_file_exists(http_client, base_url, tag, zip_sha_file.as_str()).await? {
    let sha_file = download_file_from_github(http_client, base_url, tag, zip_sha_file.as_str(), plugin_dir, display_progress).await?;
    check_sha(&file, &sha_file)?;
    fs::remove_file(sha_file)?;
  }

  unzip_file(&file, plugin_dir)
}

/// Downloads a plugin tar gz file from GitHub and installs it
pub async fn download_tar_gz_file(
  plugin_dir: &PathBuf,
  http_client: &Client,
  base_url: &str,
  tag: &String,
  tar_gz_file: String,
  tar_gz_sha_file: String,
  display_progress: bool
) -> anyhow::Result<PathBuf> {
  debug!(file = %tar_gz_file, "Found a tar gz file");
  let file = download_file_from_github(http_client, base_url, tag, tar_gz_file.as_str(), plugin_dir, display_progress).await?;

  if github_file_exists(http_client, base_url, tag, tar_gz_sha_file.as_str()).await? {
    let sha_file = download_file_from_github(http_client, base_url, tag, tar_gz_sha_file.as_str(), plugin_dir, display_progress).await?;
    check_sha(&file, &sha_file)?;
    fs::remove_file(sha_file)?;
  }

  extract_tar_gz(&file, plugin_dir)
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

fn extract_tar_gz(tar_gz_file: &PathBuf, plugin_dir: &PathBuf) -> anyhow::Result<PathBuf> {
  let file = File::open(tar_gz_file)?;
  let gz_decoder = GzDecoder::new(file);
  let mut archive = Archive::new(gz_decoder);

  archive.unpack(plugin_dir)?;
  debug!("Unpacked {:?} plugin", tar_gz_file);
  fs::remove_file(tar_gz_file)?;
  Ok(tar_gz_file.clone())
}

/// Downloads a file from GitHub showing console progress
pub async fn download_file_from_github(
  http_client: &Client,
  base_url: &str,
  tag: &String,
  filename: &str,
  plugin_dir: &PathBuf,
  display_progress: bool
) -> anyhow::Result<PathBuf> {
  let url = format!("{}/download/{}/{}", base_url, tag, filename);
  debug!("Downloading file from {}", url);

  let res = http_client.get(url.as_str()).send().await?;
  let total_size = res.content_length()
    .ok_or(anyhow!("Failed to get content length from '{}'", url))?;

  let pb = ProgressBar::new(total_size);
  if display_progress {
    pb.set_style(
      ProgressStyle::with_template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("#>-"));
    pb.set_message(format!("Downloading {}", url));
  }

  let path = plugin_dir.join(filename);
  let mut file = File::create(path.clone())?;
  let mut downloaded: u64 = 0;
  let mut stream = res.bytes_stream();

  while let Some(item) = stream.next().await {
    let chunk = item?;
    file.write_all(&chunk)?;
    let new = min(downloaded + (chunk.len() as u64), total_size);
    downloaded = new;
    if display_progress {
      pb.set_position(new);
    }
  }

  if display_progress {
    pb.finish_with_message(format!("Downloaded {} to {}", url, path.display()));
  }
  debug!(url, downloaded_bytes = downloaded, "File downloaded OK");
  Ok(path.clone())
}

/// Validates a file against a SHA file
pub fn check_sha(file: &PathBuf, sha_file: &PathBuf) -> anyhow::Result<()> {
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
