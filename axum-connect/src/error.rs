use axum::http::StatusCode;
use base_62::base62;
use prost::Message;
use serde::Serialize;

use crate::{prelude::RpcResult, response::RpcIntoResponse};

#[derive(Clone, Serialize)]
pub struct RpcError {
    pub code: RpcErrorCode,
    pub message: String,
    pub details: Vec<RpcErrorDetail>,
}

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

#[derive(Clone, Serialize)]
pub struct RpcErrorDetail {
    #[serde(rename = "type")]
    pub proto_type: String,
    #[serde(rename = "value")]
    pub proto_b62_value: String,
    #[serde(rename = "debug")]
    pub debug_json: Box<serde_json::value::RawValue>,
}

// impl<M> From<M> for RpcErrorDetail
// where
//     M: Message + Serialize,
// {
//     fn from(val: M) -> Self {
//         let binary = M::encode_to_vec(&val.1);
//         // Encode as base62
//         let b62 = base62::encode(&binary);
//         let json = serde_json::to_string(&val.1).unwrap();

//         Self {
//             M::
//             proto_b62_value: b62,
//             debug_json: serde_json::value::RawValue::from_string(json).unwrap(),
//         }
//     }
// }

#[derive(Clone, Serialize)]
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

impl From<RpcErrorCode> for StatusCode {
    fn from(val: RpcErrorCode) -> Self {
        match val {
            // Spec: https://connect.build/docs/protocol/#error-codes
            RpcErrorCode::Canceled => StatusCode::REQUEST_TIMEOUT,
            RpcErrorCode::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
            RpcErrorCode::InvalidArgument => StatusCode::BAD_REQUEST,
            RpcErrorCode::DeadlineExceeded => StatusCode::REQUEST_TIMEOUT,
            RpcErrorCode::NotFound => StatusCode::NOT_FOUND,
            RpcErrorCode::AlreadyExists => StatusCode::CONFLICT,
            RpcErrorCode::PermissionDenied => StatusCode::FORBIDDEN,
            RpcErrorCode::ResourceExhausted => StatusCode::TOO_MANY_REQUESTS,
            RpcErrorCode::FailedPrecondition => StatusCode::PRECONDITION_FAILED,
            RpcErrorCode::Aborted => StatusCode::CONFLICT,
            RpcErrorCode::OutOfRange => StatusCode::BAD_REQUEST,
            RpcErrorCode::Unimplemented => StatusCode::NOT_FOUND,
            RpcErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            RpcErrorCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            RpcErrorCode::DataLoss => StatusCode::INTERNAL_SERVER_ERROR,
            RpcErrorCode::Unauthenticated => StatusCode::UNAUTHORIZED,
        }
    }
}

impl<T> RpcIntoResponse<T> for RpcErrorCode
where
    T: Message,
{
    fn rpc_into_response(self) -> RpcResult<T> {
        Err(RpcError::new(self, "".to_string()))
    }
}

impl<T> RpcIntoResponse<T> for RpcError
where
    T: Message,
{
    fn rpc_into_response(self) -> RpcResult<T> {
        Err(self)
    }
}
