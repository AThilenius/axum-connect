use std::net::SocketAddr;

use axum::{extract::Host, Router};
use axum_connect::{error::RpcError, prelude::*};
use proto::hello::{HelloRequest, HelloResponse, HelloWorldService};

mod proto {
    include!(concat!(env!("OUT_DIR"), "/connect_proto_gen/mod.rs"));
}

#[tokio::main]
async fn main() {
    // Build our application with a route. Note the `rpc` method which was added by `axum-connect`.
    // It expect a service method handler, wrapped in it's respective type. The handler (below) is
    // just a normal Rust function. Just like Axum, it also supports extractors!
    let app = Router::new()
        .rpc(HelloWorldService::say_hello(say_hello_success))
        .rpc(HelloWorldService::say_hello(say_hello_error))
        .rpc(HelloWorldService::say_hello(say_hello_result))
        .rpc(HelloWorldService::say_hello(say_hello_error_code));

    // Axum boilerplate to start the server.
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn say_hello_success(Host(host): Host, request: HelloRequest) -> HelloResponse {
    HelloResponse {
        message: format!(
            "Hello {}! You're addressing the hostname: {}.",
            request.name, host
        ),
        special_fields: Default::default(),
    }
}

async fn say_hello_error(_request: HelloRequest) -> RpcError {
    RpcError::new(RpcErrorCode::Unimplemented, "Not implemented".to_string())
}

async fn say_hello_error_code(_request: HelloRequest) -> RpcErrorCode {
    RpcErrorCode::Unimplemented
}

async fn say_hello_result(_request: HelloRequest) -> RpcResult<HelloResponse> {
    Ok(HelloResponse {
        message: "Hello World!".to_string(),
        special_fields: Default::default(),
    })
}
