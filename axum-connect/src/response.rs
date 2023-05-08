use prost::Message;

use crate::error::{RpcError, RpcIntoError};

pub type RpcResult<M> = Result<M, RpcError>;

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
