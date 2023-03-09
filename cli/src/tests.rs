use crate::Cli;

#[test]
fn verify_cli() {
  use clap::CommandFactory;
  Cli::command().debug_assert()
}

#[test]
fn cli_tests() {
  trycmd::TestCases::new()
    .case("tests/cmd/*.toml")
    .case("README.md");
}
