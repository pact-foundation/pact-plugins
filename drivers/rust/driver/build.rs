use std::env;

use tonic_prost_build::compile_protos;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  if env::var_os("PACT_PLUGIN_BUILD_PROTOBUFS").is_some() {
    compile_protos("./plugin.proto")?
  }
  Ok(())
}
