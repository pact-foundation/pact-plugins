use crate::Cli;

#[test]
fn verify_cli() {
  use clap::CommandFactory;
  Cli::command().debug_assert()
}
