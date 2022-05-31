use std::{env, fs};
use std::cmp::Ordering;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use comfy_table::presets::UTF8_FULL;
use comfy_table::Table;
use itertools::Itertools;
use pact_plugin_driver::plugin_models::PactPluginManifest;

#[derive(Parser, Debug)]
#[clap(version, about)]
#[clap(propagate_version = true)]
struct Cli {
  #[clap(subcommand)]
  command: Commands
}

#[derive(Subcommand, Debug)]
enum Commands {
  /// List installed plugins
  List,

  /// Print out the Pact plugin environment config
  Env,

  /// Install a plugin
  Install,

  /// Remove a plugin
  Remove,

  /// Enable a plugin version
  Enable {
    /// Plugin name
    name: String,

    /// Plugin version. Not required if there is only one plugin version.
    version: Option<String>
  },

  /// Disable a plugin version
  Disable {
    /// Plugin name
    name: String,

    /// Plugin version. Not required if there is only one plugin version.
    version: Option<String>
  }
}

fn main() -> anyhow::Result<()> {
  let cli = Cli::parse();

  match &cli.command {
    Commands::List => list_plugins(),
    Commands::Env => print_env(),
    Commands::Install => Ok(()),
    Commands::Remove => Ok(()),
    Commands::Enable { name, version } => enable_plugin(name, version),
    Commands::Disable { name, version } => disable_plugin(name, version)
  }
}

fn disable_plugin(name: &String, version: &Option<String>) -> anyhow::Result<()> {
  let vec = plugin_list()?;
  let matches = vec.iter().filter(|(manifest, _, _)| {
    if let Some(version) = version {
      manifest.name == *name && manifest.version == *version
    } else {
      manifest.name == *name
    }
  }).collect_vec();
  if matches.len() == 1 {
    if let Some((manifest, file, status)) = matches.first() {
      if !*status {
        println!("Plugin '{}' with version '{}' is already disabled.", manifest.name, manifest.version);
      } else {
        fs::rename(file, file.with_file_name("pact-plugin.json.disabled"))?;
        println!("Plugin '{}' with version '{}' is now disabled.", manifest.name, manifest.version);
      }
      Ok(())
    } else {
      Err(anyhow!("Internal error, matches.len() == 1 but first() == None"))
    }
  } else if matches.len() > 1 {
    Err(anyhow!("There is more than one plugin version for '{}', please also provide the version", name))
  } else if let Some(version) = version {
    Err(anyhow!("Did not find a plugin with name '{}' and version '{}'", name, version))
  } else {
    Err(anyhow!("Did not find a plugin with name '{}'", name))
  }
}

fn enable_plugin(name: &String, version: &Option<String>) -> anyhow::Result<()> {
  let vec = plugin_list()?;
  let matches = vec.iter().filter(|(manifest, _, _)| {
    if let Some(version) = version {
      manifest.name == *name && manifest.version == *version
    } else {
      manifest.name == *name
    }
  }).collect_vec();
  if matches.len() == 1 {
    if let Some((manifest, file, status)) = matches.first() {
      if *status {
        println!("Plugin '{}' with version '{}' is already enabled.", manifest.name, manifest.version);
      } else {
        fs::rename(file, file.with_file_name("pact-plugin.json"))?;
        println!("Plugin '{}' with version '{}' is now enabled.", manifest.name, manifest.version);
      }
      Ok(())
    } else {
      Err(anyhow!("Internal error, matches.len() == 1 but first() == None"))
    }
  } else if matches.len() > 1 {
    Err(anyhow!("There is more than one plugin version for '{}', please also provide the version", name))
  } else if let Some(version) = version {
    Err(anyhow!("Did not find a plugin with name '{}' and version '{}'", name, version))
  } else {
    Err(anyhow!("Did not find a plugin with name '{}'", name))
  }
}

fn list_plugins() -> anyhow::Result<()> {
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

fn plugin_list() -> anyhow::Result<Vec<(PactPluginManifest, PathBuf, bool)>> {
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

fn print_env() -> anyhow::Result<()> {
  let mut table = Table::new();

  let (plugin_src, plugin_dir) = resolve_plugin_dir();

  table
    .load_preset(UTF8_FULL)
    .set_header(vec!["Configuration", "Source", "Value"])
    .add_row(vec!["Plugin Directory", plugin_src.as_str(), plugin_dir.as_str()]);

  println!("{table}");

  Ok(())
}

fn resolve_plugin_dir() -> (String, String) {
  let home_dir = home::home_dir()
    .map(|dir| dir.join(".pact/plugins"))
    .unwrap_or_default();
  match env::var_os("PACT_PLUGIN_DIR") {
    None => ("$HOME/.pact/plugins".to_string(), home_dir.display().to_string()),
    Some(dir) => {
      let plugin_dir = dir.to_string_lossy();
      if plugin_dir.is_empty() {
        ("$HOME/.pact/plugins".to_string(), home_dir.display().to_string())
      } else {
        ("$PACT_PLUGIN_DIR".to_string(), plugin_dir.to_string())
      }
    }
  }
}
