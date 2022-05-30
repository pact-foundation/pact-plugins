use clap::{Parser, Subcommand};

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
    Commands::Env => {}
    Commands::Install => {}
    Commands::Remove => {}
    Commands::Enable => {}
    Commands::Disable => {}
  }
}
