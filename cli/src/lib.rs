use std::{env, fs};
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::anyhow;
use clap::{ArgMatches, FromArgMatches, Parser, Subcommand};
use comfy_table::presets::UTF8_FULL;
use comfy_table::Table;
use itertools::Itertools;
use pact_plugin_driver::plugin_models::PactPluginManifest;
use requestty::OnEsc;
use tracing::{error, Level};
use tracing_subscriber::FmtSubscriber;

use crate::list::{list_plugins, plugin_list};

mod install;
mod repository;
mod list;

#[derive(Parser, Debug)]
#[clap(about, version)]
#[command(disable_version_flag(true))]
pub struct Cli {
  #[clap(short, long)]
  /// Automatically answer Yes for all prompts
  yes: bool,

  #[clap(short, long)]
  /// Enable debug level logs
  pub debug: bool,

  #[clap(short, long)]
  /// Enable trace level logs
  pub trace: bool,

  #[clap(subcommand)]
  command: Commands,

  #[clap(short = 'v', long = "version", action = clap::ArgAction::Version)]
  /// Print CLI version
  cli_version: Option<bool>
}

#[derive(Subcommand, Debug)]
enum Commands {
  /// List installed or available plugins
  #[command(subcommand)]
  List(ListCommands),

  /// Print out the Pact plugin environment config
  Env,

  /// Install a plugin
  ///
  /// A plugin can be either installed from a URL, or for a known plugin, by name (and optionally
  /// version).
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

    /// Where to fetch the plugin files from. This should be a URL or the name of a known plugin.
    source: String,

    #[clap(short, long)]
    /// The version to install. This is only used for known plugins.
    version: Option<String>,

    #[clap(long,env="PACT_PLUGIN_CLI_SKIP_LOAD")]
    /// Skip auto-loading of plugin
    skip_load: bool
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
pub enum ListCommands {
  /// List installed plugins
  Installed,

  /// List known plugins
  Known {
    /// Display all versions of the known plugins
    #[clap(short, long)]
    show_all_versions: bool
  }
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
  AddAllPluginVersions {
    /// Repository index file to update
    repository_file: String,

    /// Repository owner to load versions from
    owner: String,

    /// Repository to load versions from
    repository: String,

    /// Base URL for GitHub APIs, will default to https://api.github.com/repos/
    base_url: Option<String>
  },

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
  File { repository_file: String, file: String },

  /// Add an entry for a GitHub Release to the repository file
  GitHub { repository_file: String, url: String }
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

pub fn setup_logger(log_level: Level) {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(log_level)
    .finish();

  if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
    eprintln!("WARN: Failed to initialise global tracing subscriber - {err}");
  };
}

pub fn process_plugin_command(matches: &ArgMatches) -> Result<(), ExitCode> {
  // Convert ArgMatches into Cli by using Cli::from_arg_matches
  match Cli::from_arg_matches(matches) {
    Ok(cli) => handle_matches(&cli),
    Err(err) => {
      error!("Failed to parse arguments: {}", err);
      Err(ExitCode::FAILURE)
    }
  }
}


pub fn handle_matches(cli: &Cli) -> Result<(), ExitCode> {
  let result = match &cli.command {
    Commands::List(command) => list_plugins(command),
    Commands::Env => print_env(),
    Commands::Install { yes, skip_if_installed, source, source_type, version, skip_load } => {
      install::install_plugin(source, source_type, *yes || cli.yes, *skip_if_installed, version, *skip_load)
    },
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
