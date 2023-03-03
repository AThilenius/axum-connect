use std::net::SocketAddr;

use axum::{extract::Host, Router};
use axum_connect::*;
use proto::hello::{HelloRequest, HelloResponse, HelloWorldService};

mod proto {
    include!(concat!(env!("OUT_DIR"), "/connect_proto_gen/mod.rs"));
}

#[tokio::main]
async fn main() {
    // Build our application with a route
    let app = Router::new().rpc(HelloWorldService::say_hello(say_hello_handler));

    // Run the Axum server.
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn say_hello_handler(Host(host): Host, request: HelloRequest) -> HelloResponse {
    HelloResponse {
        message: format!(
            "Hello {}! You're addressing the hostname: {}.",
            request.name, host
        ),
        special_fields: Default::default(),
    }
}
