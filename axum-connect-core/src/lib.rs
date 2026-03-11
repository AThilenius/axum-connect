pub mod codec;
pub mod error;
pub mod response;

// Re-export several crates so that generated code can reference them
// via `axum_connect::prost::`, `axum_connect::serde::`, etc.
pub use futures;
pub use pbjson;
pub use pbjson_types;
pub use prost;
pub use serde;

/// Common result type alias for Connect RPC operations.
pub use codec::RpcResult;
