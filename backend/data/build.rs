use std::io::Result;
fn main() -> Result<()> {
    // let protos = std::fs::read_dir("../proto")
    //     .expect("Failed to read proto directory")
    //     .map(|e| e.expect("Failed to read proto file").path())
    //     .collect::<Vec<_>>();
    let protos = vec!["../../proto/httpdownload.proto"];
    prost_build::compile_protos(&protos, &["../../proto/"])?;
    Ok(())
}
