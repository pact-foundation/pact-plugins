#[test]
fn cli_tests() {
  trycmd::TestCases::new()
    .case("tests/cmd/*.toml")
    .insert_var("[CLIVERSION]", clap::crate_version!()).unwrap()
    .case("README.md");
}
