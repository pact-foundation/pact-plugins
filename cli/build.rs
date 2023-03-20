use std::fs;
use std::env;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  println!("cargo:rerun-if-changed=../repository/repository.index");
  let out_dir = env::var("OUT_DIR")?;
  let out_dir = Path::new(&out_dir);
  let path = out_dir.join("repository.index");
  fs::copy("../repository/repository.index", path)?;
  Ok(())
}
