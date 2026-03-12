pub mod error;
pub mod handler;
pub mod parts;
pub mod response;
pub mod router;

// Re-export the core crate so generated code and consumers can access
// shared types through `axum_connect::`.
pub use axum_connect_core;
pub use axum_connect_core::codec;

// Re-export several crates (originally direct, now via core for consistency)
pub use futures;
pub use pbjson;
pub use pbjson_types;
pub use prost;
pub use serde;

pub mod prelude {
    pub use crate::error::*;
    pub use crate::parts::*;
    pub use crate::response::*;
    pub use crate::router::RpcRouterExt;
}
