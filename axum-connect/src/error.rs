use axum::http::StatusCode;

// Re-export core error types so existing code using `axum_connect::error::*` still works.
pub use axum_connect_core::error::*;
pub use axum_connect_core::codec::RpcResult;

/// Convert an `RpcErrorCode` to the corresponding HTTP status code per the Connect spec.
pub fn rpc_error_code_to_status(code: &RpcErrorCode) -> StatusCode {
    match code {
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
