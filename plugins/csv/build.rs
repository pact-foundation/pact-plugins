fn main() -> Result<(), Box<dyn std::error::Error>> {
  tonic_prost_build::compile_protos("proto/plugin_v2.proto")?;
  Ok(())
}
