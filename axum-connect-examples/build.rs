use axum_connect_build::axum_connect_codegen;

fn main() {
    axum_connect_codegen("proto", &["proto/hello.proto"]).unwrap();
}
