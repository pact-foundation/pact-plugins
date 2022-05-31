use std::env;
use clap::{Parser, Subcommand};
use comfy_table::presets::UTF8_FULL;
use comfy_table::Table;

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

fn main() {
  let cli = Cli::parse();

  match &cli.command {
    Commands::List => {}
    Commands::Env => print_env(),
    Commands::Install => {}
    Commands::Remove => {}
    Commands::Enable => {}
    Commands::Disable => {}
  }
}

fn print_env() {
  let mut table = Table::new();

  let (plugin_src, plugin_dir) = resolve_plugin_dir();

  table
    .load_preset(UTF8_FULL)
    .set_header(vec!["Configuration", "Source", "Value"])
    .add_row(vec!["Plugin Directory", plugin_src.as_str(), plugin_dir.as_str()]);

  println!("{table}");
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
