//! Example Connect RPC client.
//!
//! Start the server first:
//!   $ cargo run -p axum-connect-example --bin main
//!
//! Then run this client:
//!   $ cargo run -p axum-connect-example --bin client

use axum_connect_client::ConnectClient;
use futures::StreamExt;

mod proto {
    pub mod hello {
        include!(concat!(env!("OUT_DIR"), "/hello.rs"));
    }
}

use proto::hello::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the Connect client pointing at the local server.
    let client = ConnectClient::new("http://localhost:3030");

    // The generated `HelloWorldServiceClient` gives you a typed method
    // for every RPC defined in the proto file.
    let svc = HelloWorldServiceClient::new(client);

    // ── Unary call ──────────────────────────────────────────────────
    println!("--- Unary RPC: SayHello ---");
    let response = svc
        .say_hello(HelloRequest {
            name: Some("World".into()),
        })
        .await?;
    println!("Response: {}", response.message);

    // ── Server-streaming call ───────────────────────────────────────
    println!("\n--- Server-streaming RPC: SayHelloStream ---");
    let mut stream = svc
        .say_hello_stream(HelloRequest {
            name: Some("Streamer".into()),
        })
        .await?;

    while let Some(item) = stream.next().await {
        match item {
            Ok(response) => println!("Stream message: {}", response.message),
            Err(e) => eprintln!("Stream error: {}", e),
        }
    }
    println!("Stream complete.");

    // ── Binary encoding ─────────────────────────────────────────────
    println!("\n--- Unary RPC with binary encoding ---");
    let binary_client = ConnectClient::builder("http://localhost:3030")
        .use_binary(true)
        .build();
    let binary_svc = HelloWorldServiceClient::new(binary_client);
    let response = binary_svc
        .say_hello(HelloRequest {
            name: Some("Binary World".into()),
        })
        .await?;
    println!("Response (binary): {}", response.message);

    Ok(())
}
