use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  if env::var_os("PACT_PLUGIN_BUILD_PROTOBUFS").is_some() {
    tonic_prost_build::configure().compile_protos(
      &[
        "../../../proto/plugin.proto",
        "../../../proto/plugin_v2.proto",
      ],
      &["../../../proto"],
    )?
  }
  Ok(())
}
