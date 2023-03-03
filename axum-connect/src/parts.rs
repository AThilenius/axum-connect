use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Host, Query},
    http::{self},
};
use protobuf::MessageFull;
use serde::de::DeserializeOwned;

use crate::error::{RpcError, RpcErrorCode, RpcIntoError};

#[async_trait]
pub trait RpcFromRequestParts<T, S>: Sized
where
    T: MessageFull,
    S: Send + Sync,
{
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: RpcIntoError;

    /// Perform the extraction.
    async fn rpc_from_request_parts(
        parts: &mut http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection>;
}

/// Macro to convert standard Axum `FromRequestParts` into `RpcFromRequestParts` by transforming
/// their error type.
macro_rules! impl_rpc_from_request_parts {
    ($t:ident,  $code:expr) => {
        #[async_trait]
        impl<M, S> RpcFromRequestParts<M, S> for $t
        where
            M: MessageFull,
            S: Send + Sync,
        {
            type Rejection = RpcError;

            async fn rpc_from_request_parts(
                parts: &mut http::request::Parts,
                state: &S,
            ) -> Result<Self, Self::Rejection> {
                Ok($t::from_request_parts(parts, state)
                    .await
                    .map_err(|e| ($code, e.to_string()).rpc_into_error())?)
            }
        }
    };
    ([$($tin:ident),*], $t:ident,  $code:expr) => {
        #[async_trait]
        impl<M, S, $($tin,)*> RpcFromRequestParts<M, S> for $t<$($tin,)*>
        where
            M: MessageFull,
            S: Send + Sync,
            $( $tin: DeserializeOwned, )*
        {
            type Rejection = RpcError;

            async fn rpc_from_request_parts(
                parts: &mut http::request::Parts,
                state: &S,
            ) -> Result<Self, Self::Rejection> {
                Ok($t::from_request_parts(parts, state)
                    .await
                    .map_err(|e| ($code, e.to_string()).rpc_into_error())?)
            }
        }
    };
}

impl_rpc_from_request_parts!(Host, RpcErrorCode::Internal);
impl_rpc_from_request_parts!([T], Query, RpcErrorCode::Internal);
