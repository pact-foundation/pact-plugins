use std::path::PathBuf;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let proto_file = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
    .join("../../../proto/plugin.proto").canonicalize()?;
  tonic_build::compile_protos(proto_file.to_string_lossy().to_string())?;
  Ok(())
}
