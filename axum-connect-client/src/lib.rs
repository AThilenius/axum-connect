//! Typesafe Connect-Web RPC client built on `reqwest`.
//!
//! This crate provides `ConnectClient`, a generic HTTP client for calling
//! Connect RPC services. Generated client stubs (from `axum-connect-build`)
//! use this client to make strongly-typed unary and server-streaming calls.
//!
//! # Example
//!
//! ```rust,ignore
//! use axum_connect_client::ConnectClient;
//!
//! let client = ConnectClient::new("http://localhost:3030");
//! let svc = HelloWorldServiceClient::new(client.clone());
//! let resp = svc.say_hello(HelloRequest { name: Some("World".into()) }).await?;
//! ```

pub mod stream;
pub mod transport;

// Re-export core so generated client code can reach types via `axum_connect_client::`.
pub use axum_connect_core;
pub use axum_connect_core::codec;
pub use axum_connect_core::error;
pub use axum_connect_core::response;

// Re-export key crates for generated code
pub use axum_connect_core::futures;
pub use axum_connect_core::pbjson;
pub use axum_connect_core::pbjson_types;
pub use axum_connect_core::prost;
pub use axum_connect_core::serde;

pub use transport::ConnectClient;
