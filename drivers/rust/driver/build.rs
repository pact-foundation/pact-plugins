use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  if env::var_os("PACT_PLUGIN_BUILD_PROTOBUFS").is_some() {
    tonic_build::compile_protos("./plugin.proto")?;
  }
  Ok(())
}
