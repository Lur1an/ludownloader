use std::io::Result;
fn main() -> Result<()> {
    let mut config = prost_build::Config::new();
    config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    let protos = std::fs::read_dir("../../proto")
        .expect("Failed to read proto directory")
        .map(|e| e.expect("Failed to read proto file").path())
        .collect::<Vec<_>>();
    config.compile_protos(&protos, &["../../proto/"])?;
    Ok(())
}
