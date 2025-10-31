use std::process::ExitCode;

use tracing::Level;
use pact_plugin_cli::{Cli, handle_matches, setup_logger};
use clap::Parser;

fn main() -> Result<(), ExitCode> {
  let cli = Cli::parse();

  let log_level = if cli.trace {
    Level::TRACE
  } else if cli.debug {
    Level::DEBUG
  } else {
    Level::WARN
  };
  setup_logger(log_level);
  handle_matches(&cli)
}