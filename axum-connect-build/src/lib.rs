use std::{
    env,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use gen::AxumConnectServiceGenerator;

mod gen;

#[derive(Clone, Debug)]
pub struct AxumConnectGenSettings {
    pub includes: Vec<PathBuf>,
    pub inputs: Vec<PathBuf>,
    pub protoc_args: Vec<String>,
    pub protoc_version: Option<String>,
}

impl Default for AxumConnectGenSettings {
    fn default() -> Self {
        Self {
            includes: Default::default(),
            inputs: Default::default(),
            protoc_args: Default::default(),
            protoc_version: Some("22.3".to_string()),
        }
    }
}

impl AxumConnectGenSettings {
    pub fn from_directory_recursive<P>(path: P) -> anyhow::Result<Self>
    where
        P: Into<PathBuf>,
    {
        let path = path.into();
        let mut settings = Self::default();
        settings.includes.push(path.clone());

        // Recursively add all files that end in ".proto" to the inputs.
        let mut dirs = vec![path];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path.clone());
                } else if path.extension().map(|ext| ext == "proto").unwrap_or(false) {
                    settings.inputs.push(path);
                }
            }
        }

        Ok(settings)
    }
}

pub fn axum_connect_codegen(settings: AxumConnectGenSettings) -> anyhow::Result<()> {
    // Fetch protoc
    if let Some(version) = &settings.protoc_version {
        let out_dir = env::var("OUT_DIR").unwrap();
        let protoc_path = protoc_fetcher::protoc(version, Path::new(&out_dir))?;
        env::set_var("PROTOC", protoc_path);
    }

    // Instruct cargo to re-run if any of the proto files change
    for input in &settings.inputs {
        println!("cargo:rerun-if-changed={}", input.display());
    }

    let descriptor_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("proto_descriptor.bin");

    let mut conf = prost_build::Config::new();

    // Standard prost configuration
    conf.compile_well_known_types();
    conf.file_descriptor_set_path(&descriptor_path);
    conf.extern_path(".google.protobuf", "::pbjson_types");
    conf.service_generator(Box::new(AxumConnectServiceGenerator::new()));

    // Arg configuration
    for arg in settings.protoc_args {
        conf.protoc_arg(arg);
    }

    // File configuration
    conf.compile_protos(&settings.inputs, &settings.includes)
        .unwrap();

    // Use pbjson to generate the Serde impls, and inline them with the Prost files.
    let descriptor_set = std::fs::read(descriptor_path)?;
    let mut output: PathBuf = PathBuf::from(env::var("OUT_DIR").unwrap());
    output.push("FILENAME");

    let writers = pbjson_build::Builder::new()
        .register_descriptors(&descriptor_set)?
        .generate(&["."], move |package| {
            output.set_file_name(format!("{}.rs", package));

            let file = std::fs::OpenOptions::new().append(true).open(&output)?;

            Ok(BufWriter::new(file))
        })?;

    for (_, mut writer) in writers {
        writer.flush()?;
    }

    Ok(())
}
