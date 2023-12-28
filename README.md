# Axum Connect-Web

Brings the protobuf-based [Connect-Web RPC
framework](https://connect.build/docs/introduction) to Rust via idiomatic
[Axum](https://github.com/tokio-rs/axum).

# Axum Version

- `axum-connect:0.3` works with `axum:0.7`
- `axum-connect:0.2` works with `axum:0.6`

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

# Caution ⚠️

We use `axum-connect` in production, but I don't kow that anyone with more sense
does. It's written in Rust which obviously offers some amazing compiler
guarantees, but it's not well tested or battle-proven yet. Do what you will with
that information.

Please let me know if you're using `axum-connect`! And open issues if you find a
bug.

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
cargo add axum-connect prost axum
cargo add tokio --features full
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
use std::net::SocketAddr;

use axum::{extract::Host, Router};
use axum_connect::prelude::*;
use proto::hello::*;

mod proto {
    pub mod hello {
        include!(concat!(env!("OUT_DIR"), "/hello.rs"));
    }
}

#[tokio::main]
async fn main() {
    // Build our application with a route. Note the `rpc` method which was added
    // by `axum-connect`. It expect a service method handler, wrapped in it's
    // respective type. The handler (below) is just a normal Rust function. Just
    // like Axum, it also supports extractors!
    let app = Router::new().rpc(HelloWorldService::say_hello(say_hello_success));

    // Axum boilerplate to start the server.
    let addr = SocketAddr::from(([127, 0, 0, 1], 3030));
    println!("listening on http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn say_hello_success(
  Host(host): Host,
  request: HelloRequest
) -> HelloResponse {
    HelloResponse {
        message: format!(
            "Hello {}! You're addressing the hostname: {}.",
            request.name, host
        ),
    }
}
```

## SEND IT 🚀

To test it out, try hitting the endpoint manually.

```sh
curl --location 'http://localhost:3030/hello.HelloWorldService/SayHello' \
--header 'Content-Type: application/json' \
--data '{ "name": "Alec" }'
```

From here you can stand up a `connect-web` TypeScript/Go project to call your
API with end-to-end typed RPCs.

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
