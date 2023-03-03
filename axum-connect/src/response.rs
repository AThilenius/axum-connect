use axum::response::{IntoResponse, Response};
use protobuf::MessageFull;

use crate::error::{RpcError, RpcErrorCode, RpcIntoError};

pub type RpcResult<T> = Result<T, RpcError>;

pub struct RpcResponse<T> {
    pub(crate) response: RpcResult<T>,
    pub(crate) parts: Response,
}

impl<T> IntoResponse for RpcResponse<T>
where
    T: MessageFull,
{
    fn into_response(self) -> Response {
        let rpc_call_response: Response = {
            match self.response {
                Ok(value) => protobuf_json_mapping::print_to_string(&value)
                    .map_err(|_e| {
                        RpcError::new(
                            RpcErrorCode::Internal,
                            "Failed to serialize response".to_string(),
                        )
                    })
                    .into_response(),
                Err(e) => e.into_response(),
            }
        };

        let (parts, _) = self.parts.into_parts();
        (parts, rpc_call_response).into_response()
    }
}

pub trait RpcIntoResponse<T>: Send + Sync + 'static
where
    T: MessageFull,
{
    fn rpc_into_response(self) -> RpcResponse<T>;
}

impl<T> RpcIntoResponse<T> for T
where
    T: MessageFull,
{
    fn rpc_into_response(self) -> RpcResponse<T> {
        RpcResponse {
            response: Ok(self),
            parts: Response::default(),
        }
    }
}

impl<T, E> RpcIntoResponse<T> for Result<T, E>
where
    T: MessageFull,
    E: RpcIntoError + Send + Sync + 'static,
{
    fn rpc_into_response(self) -> RpcResponse<T> {
        match self {
            Ok(res) => res.rpc_into_response(),
            Err(err) => err.rpc_into_error().rpc_into_response(),
        }
    }
}
