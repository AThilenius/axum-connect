# Axum Connect-Web

> ⚠️ This project isn't even Alpha state yet. Don't use it.

Brings the protobuf-based [Connect-Web RPC
framework](https://connect.build/docs/introduction) to Rust via idiomatic
[Axum](https://github.com/tokio-rs/axum). That means Axum extractors are fully
supported, while maintaining strongly typed, contract-driven RPC
implementations.

## Hello, World!

_Prior knowledge with [Protobuf](https://github.com/protocolbuffers/protobuf)
(both the IDL and it's use in RPC frameworks) and
[Axum](https://github.com/tokio-rs/axum) are assumed._

Starting with the obligatory hello world proto service definition

`proto/hello.proto`

```protobuf
syntax = "proto3";

package hello_world;

message HelloRequest {
    string name = 1;
}

message HelloResponse {
    string message = 1;
}

service HelloWorldService {
    rpc SayHello(HelloRequest) returns (HelloResponse) {}
}

```

Axum-Connect code can be generated using `axum_connect_codegen`

`build.rs`

```rust
use axum_connect_build::axum_connect_codegen;

fn main() {
    axum_connect_codegen("proto", &["proto/hello.proto"]).unwrap();
}
```

Now we can implement the service contract using handlers that look and behave
much like Axum handlers.

```rust
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
```

## License

Axum-Connect is dual licensed (at your option)

- MIT License ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
