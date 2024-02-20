//!
//! $ cargo run -p axum-connect-example
//! Head over to Buf Studio to test out RPCs, with auto-completion!
//! https://buf.build/studio/athilenius/axum-connect/main/hello.HelloWorldService/SayHello?target=http%3A%2F%2Flocalhost%3A3030
//!

use async_stream::stream;
use axum::{extract::Host, Router};
use axum_connect::{futures::Stream, prelude::*};
use error::Error;
use proto::hello::*;
use tower_http::cors::CorsLayer;

// Take a peak at error.rs to see how errors work in axum-connect.
mod error;

mod proto {
    // Include the generated code in a `proto` module.
    //
    // Note: I'm not super happy with this pattern. I hope to add support to `protoc-gen-prost` in
    // the near-ish future instead see:
    // https://github.com/neoeinstein/protoc-gen-prost/issues/82#issuecomment-1877107220 That will
    // better align with Buf.build's philosophy. This is how it works for now though.
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
        // A standard unary (POST based) Connect-Web request handler.
        .rpc(HelloWorldService::say_hello(say_hello_unary))
        // A GET version of the same thing, which has well-defined semantics for caching.
        .rpc(HelloWorldService::say_hello_unary_get(say_hello_unary))
        // A server-streaming request handler. Very useful when you need them!
        .rpc(HelloWorldService::say_hello_stream(stream_three_reponses));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3030")
        .await
        .unwrap();
    println!("listening on http://{:?}", listener.local_addr().unwrap());
    axum::serve(listener, app.layer(CorsLayer::very_permissive()))
        .await
        .unwrap();
}

/// The bread-and-butter of Connect-Web, a Unary request handler.
///
/// Just to demo error handling, I've chose to return a `Result` here. If your method is
/// infallible, you could just as easily return a `HellResponse` directly. The error type I'm using
/// is defined in `error.rs` and is worth taking a quick peak at.
///
/// Like Axum, both the request AND response types just need to implement RpcFromRequestParts` and
/// `RpcIntoResponse` respectively. This allows for a ton of flexibility in what your handlers
/// actually accept/return. This is a concept very core to Axum, so I won't go too deep into the
/// ideology here.
async fn say_hello_unary(Host(host): Host, request: HelloRequest) -> Result<HelloResponse, Error> {
    Ok(HelloResponse {
        message: format!(
            "Hello {}! You're addressing the hostname: {}.",
            request.name.unwrap_or_else(|| "unnamed".to_string()),
            host
        ),
    })
}

/// This is a server-streaming request handler. Much more rare to see one in the wild, but they
/// sure are useful when you need them! axum-connect has only partial support for everything
/// connect-web defines in server-streaming requests. For example, it doesn't define a way to
/// return trailers. I've never once actually needed them, so it feels weird to muddy the API just
/// to support such a niche use. Trailers are IMO the worst single decision gRPC made, locking them
/// into HTTP/2 forever. I'm not a fan -.-
///
/// You can however return a stream of anything that converts `RpcIntoResponse`, just like the
/// unary handlers. Again, very flexible. In this case I'm using the amazing `async-stream` crate
/// to make the code nice and readable.
async fn stream_three_reponses(
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
