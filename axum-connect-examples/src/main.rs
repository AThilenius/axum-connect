use std::net::SocketAddr;

use axum::{extract::Host, Router};
use axum_connect::*;
use proto::hello::{HelloRequest, HelloResponse, HelloWorldService};

mod proto {
    include!(concat!(env!("OUT_DIR"), "/connect_proto_gen/mod.rs"));
}

#[tokio::main]
async fn main() {
    // Build our application with a route. Note the `rpc` method which was added by `axum-connect`.
    // It expect a service method handler, wrapped in it's respective type. The handler (below) is
    // just a normal Rust function. Just like Axum, it also supports extractors!
    let app = Router::new().rpc(HelloWorldService::say_hello(say_hello_handler));

    // Axum boilerplate to start the server.
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// This is the magic. This is the TYPED handler for the `say_hello` method, changes to the proto
// definition will need to be reflected here. But the first N arguments can be standard Axum
// extracts, to get at what ever info or state you need.
async fn say_hello_handler(Host(host): Host, request: HelloRequest) -> HelloResponse {
    HelloResponse {
        message: format!(
            "Hello {}! You're addressing the hostname: {}.",
            request.name, host
        ),
        special_fields: Default::default(),
    }
}
