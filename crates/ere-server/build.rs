fn main() -> Result<(), Box<dyn std::error::Error>> {
    prost_build::Config::new()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]") // enable support for JSON encoding
        .service_generator(twirp_build::service_generator())
        .compile_protos(&["./proto/api.proto"], &["./proto"])
        .expect("error compiling protos");
    Ok(())
}
