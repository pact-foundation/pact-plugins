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
  Enable,

  /// Disable a plugin version
  Disable
}

fn main() -> anyhow::Result<()> {
  let cli = Cli::parse();

  match &cli.command {
    Commands::List => list_plugins(),
    Commands::Env => print_env(),
    Commands::Install => Ok(()),
    Commands::Remove => Ok(()),
    Commands::Enable => Ok(()),
    Commands::Disable => Ok(())
  }
}

fn list_plugins() -> anyhow::Result<()> {
  let (_, plugin_dir) = resolve_plugin_dir();
  let dir = PathBuf::from(plugin_dir);
  if dir.exists() {
    let mut plugins = vec![];
    for entry in fs::read_dir(dir)? {
      let path = entry?.path();
      if path.is_dir() {
        let manifest_file = path.join("pact-plugin.json");
        if manifest_file.exists() && manifest_file.is_file() {
          let file = File::open(manifest_file)?;
          let reader = BufReader::new(file);
          let manifest: PactPluginManifest = serde_json::from_reader(reader)?;
          plugins.push((PactPluginManifest {
            plugin_dir: path.display().to_string(),
            .. manifest
          }, "enabled"));
        } else {
          let manifest_file = path.join("pact-plugin.json.disabled");
          if manifest_file.exists() && manifest_file.is_file() {
            let file = File::open(manifest_file)?;
            let reader = BufReader::new(file);
            let manifest: PactPluginManifest = serde_json::from_reader(reader)?;
            plugins.push((PactPluginManifest {
              plugin_dir: path.display().to_string(),
              .. manifest
            }, "disabled"));
          }
        }
      }
    }

    let mut table = Table::new();
    table
      .load_preset(UTF8_FULL)
      .set_header(vec!["Name", "Version", "Interface Version", "Directory", "Status"]);

    for (manifest, status) in plugins.iter().sorted_by(manifest_sort_fn) {
      table.add_row(vec![
        manifest.name.as_str(),
        manifest.version.as_str(),
        manifest.plugin_interface_version.to_string().as_str(),
        manifest.plugin_dir.to_string().as_str(),
        status
      ]);
    }

    println!("{table}");

    Ok(())
  } else {
    Err(anyhow!("Plugin directory '{}' does not exist!", dir.display()))
  }
}

fn manifest_sort_fn(a: &&(PactPluginManifest, &str), b: &&(PactPluginManifest, &str)) -> Ordering {
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
