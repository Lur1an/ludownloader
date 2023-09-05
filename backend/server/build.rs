use schema2code::deserializer::SchemaDef;
use std::{collections::HashMap, env, path::Path, process::Command};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("openapi.rs");
    let yaml_data = include_str!("../../openapi.yaml");
    let input = serde_yaml::from_str::<serde_yaml::Value>(yaml_data).unwrap();
    let input = serde_yaml::from_value::<HashMap<String, SchemaDef>>(
        input["components"]["schemas"].clone(),
    )
    .unwrap();
    let codegen = schema2code::generate_rust(input);
    std::fs::write(&dest_path, codegen).unwrap();
    Command::new("rustfmt")
        .arg(&dest_path)
        .output()
        .expect("Failed to format generated code");
}
