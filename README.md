# Axum Connect-Web

Brings the protobuf-based [Connect-Web RPC
framework](https://connect.build/docs/introduction) to Rust via idiomatic
[Axum](https://github.com/tokio-rs/axum).

# Features 🔍

- Integrates into existing Axum HTTP applications seamlessly
- Closely mirrors Axum's API
  - Extract State and other `parts` that impl `RpcFromRequestParts` just like
    with Axum.
  - Return any type that impl `RpcIntoResponse` just like Axum.
- Generated types and service handlers are strongly typed and...
- Handlers enforce semantically correct HTTP 'parts' access.
- Allows users to derive `RpcIntoResponse` and `RpcFromRequestParts` just like
  Axum.
  - Note: These must be derivatives of Axum's types because they are more
    restrictive; you're not dealing with arbitrary HTTP any more, you're
    speaking `connect-web` RPC **over** HTTP.
- Wrap `connect-web` error handling in idiomatic Axum/Rust.
- Codegen from `*.proto` files in a separate crate.
- All the other amazing benefits that come with Axum, like the community,
  documentation and performance!

# Getting Started 🤓

_Prior knowledge with [Protobuf](https://github.com/protocolbuffers/protobuf)
(both the IDL and it's use in RPC frameworks) and
[Axum](https://github.com/tokio-rs/axum) are assumed._

## Dependencies 👀

You'll need 2 `axum-connect` crates, one for code-gen and one for runtime use.
Because of how prost works, you'll also need to add it to your own project.
You'll obviously also need `axum` and `tokio`.

```sh
# Note: axum-connect-build will fetch `protoc` for you.
cargo add --build axum-connect-build

# Both Prost and Axum are version sensitive. I haven't found a good workaround
# for this yet. PRs welcome!
cargo add axum-connect tokio prost@0.13 axum@0.8 --features=tokio/full
```

## Protobuf File 🥱

Start by creating the obligatory 'hello world' proto service definition.

`proto/hello.proto`

```protobuf
syntax = "proto3";

package hello;

message HelloRequest { string name = 1; }

message HelloResponse { string message = 1; }

service HelloWorldService {
  rpc SayHello(HelloRequest) returns (HelloResponse) {}
}
```

## Codegen 🤔

Use the `axum_connect_codegen` crate to generate Rust code from the proto IDL.

> Currently all codegen is done by having the proto files locally on-disk, and
> using a `build.rs` file. Someday I hope to support more of Buf's idioms like
> [Remote Code Gen](https://buf.build/docs/bsr/remote-plugins/usage).

`build.rs`

```rust
use axum_connect_build::{axum_connect_codegen, AxumConnectGenSettings};

fn main() {
    // This helper will use `proto` as the import path, and globs all .proto
    // files in the `proto` directory.
    //
    // Note that you might need to re-save the `build.rs` file after updating
    // a proto file to get rust-analyzer to pickup the change. I haven't put
    // time into looking for a fix to that yet.
    let settings = AxumConnectGenSettings::from_directory_recursive("proto")
        .expect("failed to glob proto files");

    axum_connect_codegen(settings).unwrap();
}
```

## The Fun Part 😁

With the boring stuff out of the way, let's implement our service using Axum!

```rust
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
```

## SEND IT 🚀

```sh
cargo run -p axum-connect-example
```

It's Connect RPCs, so you can use the Buf Studio to test things out! [Buf Studio - Hello RPC](https://buf.build/studio/athilenius/axum-connect/main/hello.HelloWorldService/SayHello?target=http%3A%2F%2Flocalhost%3A3030&share=sxKoVspLzE1VslJQcsxJTVaqBQA)

Or CURL it

```sh
curl -X POST http://localhost:3030/hello.HelloWorldService/SayHello \
     -H "Content-Type: application/json" \
     -d '{"name":"Alec"}'
```

> {"message":"Hello Alec! You're addressing the hostname: localhost:3030."}

# Request/Response Parts 🙍‍♂️

Both the request and response types are derived in `axum-connect`. This might
seem redundant at first.

Let's go over the easier one first, `RpcIntoResponse`. Connect RPCs are not
arbitrary HTML requests, they cannot return arbitrary HTML responses. For
example, streaming responses MUST return an HTTP 200 regardless of error state.
Keeping with Axum's (fantastic) paradigm, that is enforced by the type system.
RPC handlers may not return arbitrary HTML, but instead must return something
that `axum-connect` knows how to turn into a valid Connect response.

Somewhat less intuitively, `axum-connect` derives `RpcFromRequestParts`, which
is _almost_ identical to Axum's `FromRequestParts`. Importantly though,
`FromRequestParts` can return back an error of arbitrary HTML responses, which
is a problem for the same reason.

Axum also allows a `FromRequest` to occupy the last parameter in a handler which
consumed the remainder of the HTTP request (including the body). `axum-connect`
needs to handle the request input itself, so there is no equivalent for RPCs
handlers.

# Roadmap / Stated Non-Goals 🛣️

- Explore better typing than `RpcFromRequestParts`
  - Ideally clients only need to provide an `RpcIntoError`, but I haven't fully
    thought this problem through. I just know that having to specific both a
    `FromRequestParts` and an `RpcFromRequestParts` on custom types is a PITA.
- Rework error responses to allow for multiple errors
- Version checking between generated and runtime code
- A plan for forward-compatibility
- Bring everything in-line with `connect-web` and...
- Comprehensive integration tests

## More Distant Goals 🌜

- I would love to also support a WASM-ready client library
- Use `buf.build` to support remote codegen and streamlined proto handling
- Support gRPC calls
  - I don't think this is hard to do, I just have no personal use-case for it
- Possibly maybe-someday support BiDi streaming over WebRTC
  - This would require `connect-web` picking up support for the same
  - WebRTC streams because they are DTLS/SRTP and are resilient
- Replace Prost (with something custom and simpler)

## Non-goals 🙅

- To support every feature gRPC does
  - You get a lot of this already with Axum, but gRPC is a monster that I
    don't wish to reproduce. That complexity is useful for Google, and gets in
    the way for pretty much everyone else.
- To do everything and integrate with everything
  - I plan on keeping `axum-connect` highly focused. Good at what it does and
    nothing else.
  - This is idiomatic Rust. Do one thing well, and leave the rest to other
    crates.

# Prost and Protobuf 📖

## Protoc Version

The installed version of `protoc` can be configured in the
`AxumConnectGenSettings` if you need/wish to do so. Setting the value to `None`
will disable the download entirely.

## Reasoning

Prost stopped shipping `protoc` binaries (a decision I disagree with) so
`axum-connect-build` internally uses
[protoc-fetcher](https://crates.io/crates/protoc-fetcher) to download and
resolve a copy of `protoc`. This is far more turnkey than forcing every build
environment (often Heroku and/or Docker) to have a recent `protoc` binary
pre-installed. This behavior can be disabled if you disagree, or if you need to
comply with corporate policy, or your build environment is offline.

I would someday like to replace all of it with a new 'lean and
mean' protoc library for the Rust community. One with a built-in parser, that
supports only the latest proto3 syntax as well as the canonical JSON
serialization format and explicitly doesn't support many of the rarely used
features. But that day is not today.

# License 🧾

Axum-Connect is dual licensed (at your option)

- MIT License ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
