use std::{
    env,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use gen::AxumConnectServiceGenerator;

mod gen;

pub fn axum_connect_codegen(
    include: &[impl AsRef<Path>],
    inputs: &[impl AsRef<Path>],
) -> anyhow::Result<()> {
    let descriptor_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("proto_descriptor.bin");

    let mut conf = prost_build::Config::new();
    conf.compile_well_known_types();
    conf.file_descriptor_set_path(&descriptor_path);
    conf.extern_path(".google.protobuf", "::pbjson_types");
    conf.service_generator(Box::new(AxumConnectServiceGenerator::new()));
    conf.compile_protos(inputs, include).unwrap();

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
