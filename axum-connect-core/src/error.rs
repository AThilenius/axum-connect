use serde::{Deserialize, Serialize};

/// A Connect RPC error, matching the Connect protocol error JSON schema.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcError {
    pub code: RpcErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<RpcErrorDetail>,
}

/// Trait for converting arbitrary error types into an `RpcError`.
pub trait RpcIntoError {
    fn rpc_into_error(self) -> RpcError;
}

impl RpcIntoError for RpcError {
    fn rpc_into_error(self) -> RpcError {
        self
    }
}

impl RpcError {
    pub fn new(code: RpcErrorCode, message: String) -> Self {
        Self {
            code,
            message,
            details: vec![],
        }
    }
}

impl<C, M> RpcIntoError for (C, M)
where
    C: Into<RpcErrorCode>,
    M: Into<String>,
{
    fn rpc_into_error(self) -> RpcError {
        RpcError {
            code: self.0.into(),
            message: self.1.into(),
            details: vec![],
        }
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC error {:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

/// A detail entry in a Connect RPC error.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcErrorDetail {
    #[serde(rename = "type")]
    pub proto_type: String,
    #[serde(rename = "value")]
    pub proto_b62_value: String,
}

/// Well-defined Connect RPC error codes, mirroring gRPC status codes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RpcErrorCode {
    Canceled,
    Unknown,
    InvalidArgument,
    DeadlineExceeded,
    NotFound,
    AlreadyExists,
    PermissionDenied,
    ResourceExhausted,
    FailedPrecondition,
    Aborted,
    OutOfRange,
    Unimplemented,
    Internal,
    Unavailable,
    DataLoss,
    Unauthenticated,
}
