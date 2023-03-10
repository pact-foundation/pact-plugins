use std::{env, fs};
use std::cmp::Ordering;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use comfy_table::presets::UTF8_FULL;
use comfy_table::Table;
use itertools::Itertools;
use pact_plugin_driver::plugin_models::PactPluginManifest;
use requestty::OnEsc;
use tracing::{error, Level};
use tracing_subscriber::FmtSubscriber;

mod install;
mod repository;

#[derive(Parser, Debug)]
#[clap(about, version)]
#[command(disable_version_flag(true))]
struct Cli {
  #[clap(short, long)]
  /// Automatically answer Yes for all prompts
  yes: bool,

  #[clap(short, long)]
  /// Enable debug level logs
  debug: bool,

  #[clap(short, long)]
  /// Enable trace level logs
  trace: bool,

  #[clap(subcommand)]
  command: Commands,

  #[clap(short = 'v', long = "version", action = clap::ArgAction::Version)]
  /// Print CLI version
  cli_version: Option<bool>
}

#[derive(Subcommand, Debug)]
enum Commands {
  /// List installed plugins
  List,

  /// Print out the Pact plugin environment config
  Env,

  /// Install a plugin
  Install {
    /// The type of source to fetch the plugin files from. Will default to Github releases.
    ///
    /// Valid values: github
    #[clap(short = 't', long)]
    source_type: Option<InstallationSource>,

    #[clap(short, long)]
    /// Automatically answer Yes for all prompts
    yes: bool,

    #[clap(short, long)]
    /// Skip installing the plugin if the same version is already installed
    skip_if_installed: bool,

    /// Where to fetch the plugin files from. This should be a URL.
    source: String
  },

  /// Remove a plugin
  Remove {
    #[clap(short, long)]
    /// Automatically answer Yes for all prompts
    yes: bool,

    /// Plugin name
    name: String,

    /// Plugin version. Not required if there is only one plugin version.
    version: Option<String>
  },

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
  },

  /// Sub-commands for dealing with a plugin repository
  #[command(subcommand)]
  Repository(RepositoryCommands)
}

#[derive(Subcommand, Debug)]
enum RepositoryCommands {
  /// Check the consistency of the repository index file
  Validate {
    /// Filename to validate
    filename: String
  },

  /// Create a new blank repository index file
  New {
    /// Filename to use for the new file. By default will use repository.index
    filename: Option<String>,

    #[clap(short, long)]
    /// Overwrite any existing file?
    overwrite: bool
  },

  /// Add a plugin version to the index file (will update existing entry)
  #[command(subcommand)]
  AddPluginVersion(PluginVersionCommand),

  /// Add all versions of a plugin to the index file (will update existing entries)
  AddAllPluginVersions,

  /// Remove a plugin version from the index file
  YankVersion,

  /// List all plugins found in the index file
  List {
    /// Filename to list entries from
    filename: String
  },

  /// List all plugin versions found in the index file
  ListVersions{
    /// Filename to list versions from
    filename: String,

    /// Plugin entry to list versions for
    name: String
  }
}

#[derive(Subcommand, Debug)]
enum PluginVersionCommand {
  /// Add an entry for a local plugin manifest file to the repository file
  File { repository_file: String, file: String }
}

/// Installation source to fetch plugins files from
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum InstallationSource {
  /// Install the plugin from a Github release page.
  Github
}

impl FromStr for InstallationSource {
  type Err = anyhow::Error;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    if s.to_lowercase() == "github" {
      Ok(InstallationSource::Github)
    } else {
      Err(anyhow!("'{}' is not a valid installation source", s))
    }
  }
}

fn main() -> Result<(), ExitCode> {
  let cli = Cli::parse();

  let log_level = if cli.trace {
    Level::TRACE
  } else if cli.debug {
    Level::DEBUG
  } else {
    Level::WARN
  };
  let subscriber = FmtSubscriber::builder()
    .with_max_level(log_level)
    .finish();

  if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
    eprintln!("WARN: Failed to initialise global tracing subscriber - {err}");
  };

  let result = match &cli.command {
    Commands::List => list_plugins(),
    Commands::Env => print_env(),
    Commands::Install { yes, skip_if_installed, source, source_type } => install::install_plugin(source, source_type, *yes || cli.yes, *skip_if_installed),
    Commands::Remove { yes, name, version } => remove_plugin(name, version, *yes || cli.yes),
    Commands::Enable { name, version } => enable_plugin(name, version),
    Commands::Disable { name, version } => disable_plugin(name, version),
    Commands::Repository(command) => repository::handle_command(command)
  };

  result.map_err(|err| {
    error!("error - {}", err);
    ExitCode::FAILURE
  })
}

fn remove_plugin(name: &String, version: &Option<String>, override_prompt: bool) -> anyhow::Result<()> {
  let matches = find_plugin(name, version)?;
  if matches.len() == 1 {
    if let Some((manifest, _, _)) = matches.first() {
      if override_prompt || prompt_delete(manifest) {
        fs::remove_dir_all(manifest.plugin_dir.clone())?;
        println!("Removed plugin with name '{}' and version '{}'", manifest.name, manifest.version);
      } else {
        println!("Aborting deletion of plugin.");
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

fn prompt_delete(manifest: &PactPluginManifest) -> bool {
  let question = requestty::Question::confirm("delete_plugin")
    .message(format!("Are you sure you want to delete plugin with name '{}' and version '{}'?", manifest.name, manifest.version))
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

fn disable_plugin(name: &String, version: &Option<String>) -> anyhow::Result<()> {
  let matches = find_plugin(name, version)?;
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

fn find_plugin(name: &String, version: &Option<String>) -> anyhow::Result<Vec<(PactPluginManifest, PathBuf, bool)>> {
  let vec = plugin_list()?;
  Ok(vec.iter()
    .filter(|(manifest, _, _)| {
      if let Some(version) = version {
        manifest.name == *name && manifest.version == *version
      } else {
        manifest.name == *name
      }
    })
    .map(|(m, p, s)| {
      (m.clone(), p.clone(), *s)
    })
    .collect_vec())
}

fn enable_plugin(name: &String, version: &Option<String>) -> anyhow::Result<()> {
  let matches = find_plugin(name, version)?;
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

#[cfg(test)]
mod tests;
