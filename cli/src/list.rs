use std::cmp::Ordering;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::anyhow;
use comfy_table::presets::UTF8_FULL;
use comfy_table::Table;
use itertools::Itertools;
use pact_plugin_driver::plugin_models::PactPluginManifest;

use crate::{ListCommands, resolve_plugin_dir};
use crate::repository::fetch_repository_index;

pub fn list_plugins(command: &ListCommands) -> anyhow::Result<()> {
  match command {
    ListCommands::Installed => list_installed_plugins(),
    ListCommands::Known { show_all_versions } => list_known_plugins(show_all_versions)
  }
}

fn list_known_plugins(show_all_versions: &bool) -> anyhow::Result<()> {
  let index = fetch_repository_index()?;

  let mut table = Table::new();
  if *show_all_versions {
    table
      .load_preset(UTF8_FULL)
      .set_header(vec!["Name", "Version", "Source", "Value"]);

    for entry in index.entries.values() {
      for version in &entry.versions {
        table.add_row(vec![
          entry.name.as_str(),
          version.version.as_str(),
          version.source.name().as_str(),
          version.source.value().as_str()
        ]);
      }
    }
  } else {
    table
      .load_preset(UTF8_FULL)
      .set_header(vec!["Name", "Latest Version", "Num Versions"]);

    for entry in index.entries.values() {
      table.add_row(vec![
        entry.name.as_str(),
        entry.latest_version.as_str(),
        entry.versions.len().to_string().as_str()
      ]);
    }
  }

  println!("{table}");

  Ok(())
}

fn list_installed_plugins() -> anyhow::Result<()> {
  let mut table = Table::new();
  table
    .load_preset(UTF8_FULL)
    .set_header(vec!["Name", "Version", "Interface Version", "Directory", "Status"]);

  for (manifest, _, status) in plugin_list()?.iter().sorted_by(manifest_sort_fn) {
    table.add_row(vec![
      manifest.name.as_str(),
      manifest.version.as_str(),
      manifest.plugin_interface_version.to_string().as_str(),
      manifest.plugin_dir.to_string().as_str(),
      if *status { "enabled" } else { "disabled" }
    ]);
  }

  println!("{table}");

  Ok(())
}

pub fn plugin_list() -> anyhow::Result<Vec<(PactPluginManifest, PathBuf, bool)>> {
  let (_, plugin_dir) = resolve_plugin_dir();
  let dir = PathBuf::from(plugin_dir);
  if dir.exists() {
    let mut plugins = vec![];
    for entry in fs::read_dir(dir)? {
      let path = entry?.path();
      if path.is_dir() {
        let manifest_file = path.join("pact-plugin.json");
        if manifest_file.exists() && manifest_file.is_file() {
          let file = File::open(manifest_file.clone())?;
          let reader = BufReader::new(file);
          let manifest: PactPluginManifest = serde_json::from_reader(reader)?;
          plugins.push((PactPluginManifest {
            plugin_dir: path.display().to_string(),
            ..manifest
          }, manifest_file.clone(), true));
        } else {
          let manifest_file = path.join("pact-plugin.json.disabled");
          if manifest_file.exists() && manifest_file.is_file() {
            let file = File::open(manifest_file.clone())?;
            let reader = BufReader::new(file);
            let manifest: PactPluginManifest = serde_json::from_reader(reader)?;
            plugins.push((PactPluginManifest {
              plugin_dir: path.display().to_string(),
              ..manifest
            }, manifest_file.clone(), false));
          }
        }
      }
    }
    Ok(plugins)
  } else {
    Err(anyhow!("Plugin directory '{}' does not exist!", dir.display()))
  }
}

fn manifest_sort_fn(a: &&(PactPluginManifest, PathBuf, bool), b: &&(PactPluginManifest, PathBuf, bool)) -> Ordering {
  if a.0.name == b.0.name {
    a.0.version.cmp(&b.0.version)
  } else {
    a.0.name.cmp(&b.0.name)
  }
}
