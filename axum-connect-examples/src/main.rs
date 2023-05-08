use std::net::SocketAddr;

use async_stream::stream;
use axum::{extract::Host, Router};
use axum_connect::{futures::Stream, prelude::*};
use proto::hello::*;

mod proto {
    pub mod hello {
        include!(concat!(env!("OUT_DIR"), "/hello.rs"));
    }
}

#[tokio::main]
async fn main() {
    // Build our application with a route. Note the `rpc` method which was added by `axum-connect`.
    // It expect a service method handler, wrapped in it's respective type. The handler (below) is
    // just a normal Rust function. Just like Axum, it also supports extractors!
    let app = Router::new()
        .rpc(HelloWorldService::say_hello(say_hello_success))
        .rpc(HelloWorldService::say_hello_stream(say_hello_stream));

    // Axum boilerplate to start the server.
    let addr = SocketAddr::from(([127, 0, 0, 1], 3030));
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
            request.name.unwrap_or_else(|| "unnamed".to_string()),
            host
        ),
    }
}

async fn say_hello_stream(
    Host(host): Host,
    request: HelloRequest,
) -> impl Stream<Item = HelloResponse> {
    stream! {
        yield HelloResponse { message: "Hello".to_string() };
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        yield HelloResponse { message: request.name().to_string() };
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        yield HelloResponse { message: format!("You're addressing the hostname: {}", host) };
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
