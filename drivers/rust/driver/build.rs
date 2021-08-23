use std::path::PathBuf;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  tonic_build::compile_protos("plugin.proto")?;
  Ok(())
}
