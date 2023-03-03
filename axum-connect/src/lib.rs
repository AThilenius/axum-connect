// Re-export protobuf and protobuf_json_mapping for downstream use.
pub use protobuf;
pub use protobuf_json_mapping;

pub mod error;
pub mod handler;
pub mod parts;
pub mod response;
pub mod router;

pub mod prelude {
    pub use crate::error::*;
    pub use crate::parts::*;
    pub use crate::response::*;
    pub use crate::router::RpcRouterExt;
}
