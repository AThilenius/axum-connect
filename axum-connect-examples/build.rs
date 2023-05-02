use axum_connect_build::{axum_connect_codegen, AxumConnectGenSettings};

fn main() {
    let settings = AxumConnectGenSettings::from_directory_recursive("proto")
        .expect("failed to glob proto files");
    axum_connect_codegen(settings).unwrap();
}
