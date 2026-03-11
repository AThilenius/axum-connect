use prost::Message;

use crate::codec::RpcResult;
use crate::error::{RpcError, RpcErrorCode, RpcIntoError};

/// Trait for converting handler return values into an RPC result.
pub trait RpcIntoResponse<T>: Send + Sync + 'static
where
    T: Message,
{
    fn rpc_into_response(self) -> RpcResult<T>;
}

impl<T> RpcIntoResponse<T> for T
where
    T: Message + 'static,
{
    fn rpc_into_response(self) -> RpcResult<T> {
        Ok(self)
    }
}

impl<T, E> RpcIntoResponse<T> for Result<T, E>
where
    T: Message + 'static,
    E: RpcIntoError + Send + Sync + 'static,
{
    fn rpc_into_response(self) -> RpcResult<T> {
        self.map_err(|e| e.rpc_into_error())
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
