pub mod error;
pub mod handler;
pub mod parts;
pub mod response;
pub mod router;

// Re-export both prost and serde.
pub use prost;
pub use serde;

pub mod prelude {
    pub use crate::error::*;
    pub use crate::parts::*;
    pub use crate::response::*;
    pub use crate::router::RpcRouterExt;
}
