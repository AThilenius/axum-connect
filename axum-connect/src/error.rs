use axum::http::StatusCode;
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
}

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

// TODO: This needs to be done in the handler to support streaming errors.
// impl IntoResponse for RpcError {
//     fn into_response(self) -> Response {
//         let status_code = StatusCode::from(self.code.clone());
//         let json = serde_json::to_string(&self).expect("serialize error type");
//         (status_code, json).into_response()
//     }
// }
