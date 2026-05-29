fn main() {
  println!("cargo:rerun-if-changed=../../proto/plugin_v2.proto");

  tonic_prost_build::configure()
    .compile_protos(&["../../proto/plugin_v2.proto"], &["../../proto"])
    .expect("failed to compile plugin_v2.proto");
}
