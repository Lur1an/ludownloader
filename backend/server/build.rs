use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    tonic_build::compile_protos("../../proto/ludownloader.proto")
        .unwrap_or_else(|e| panic!("Failed to compile protos {e:?}"));
}
