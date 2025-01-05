use async_trait::async_trait;
use axum::{
    extract::{
        connect_info::MockConnectInfo, ConnectInfo, FromRef, FromRequestParts, Query, State,
    },
    http::{self},
    Extension,
};
#[cfg(feature = "axum-extra")]
use axum_extra::extract::Host;
use prost::Message;
use serde::de::DeserializeOwned;

use crate::error::{RpcError, RpcErrorCode, RpcIntoError};

#[async_trait]
pub trait RpcFromRequestParts<T, S>: Sized
where
    T: Message,
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

#[cfg(feature = "axum-extra")]
#[async_trait]
impl<M, S> RpcFromRequestParts<M, S> for Host
where
    M: Message,
    S: Send + Sync,
{
    type Rejection = RpcError;

    async fn rpc_from_request_parts(
        parts: &mut http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(Host::from_request_parts(parts, state)
            .await
            .map_err(|e| (RpcErrorCode::Internal, e.to_string()).rpc_into_error())?)
    }
}

#[async_trait]
impl<M, S, T> RpcFromRequestParts<M, S> for Query<T>
where
    M: Message,
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = RpcError;

    async fn rpc_from_request_parts(
        parts: &mut http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(Query::from_request_parts(parts, state)
            .await
            .map_err(|e| (RpcErrorCode::Internal, e.to_string()).rpc_into_error())?)
    }
}

#[async_trait]
impl<M, S, T> RpcFromRequestParts<M, S> for ConnectInfo<T>
where
    M: Message,
    S: Send + Sync,
    T: Clone + Send + Sync + 'static,
{
    type Rejection = RpcError;

    async fn rpc_from_request_parts(
        parts: &mut http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        match Extension::<Self>::from_request_parts(parts, state).await {
            Ok(Extension(connect_info)) => Ok(connect_info),
            Err(err) => match parts.extensions.get::<MockConnectInfo<T>>() {
                Some(MockConnectInfo(connect_info)) => Ok(Self(connect_info.clone())),
                None => Err((RpcErrorCode::Internal, err.to_string()).rpc_into_error()),
            },
        }
    }
}

#[async_trait]
impl<M, OuterState, InnerState> RpcFromRequestParts<M, OuterState> for State<InnerState>
where
    M: Message,
    InnerState: FromRef<OuterState>,
    OuterState: Send + Sync,
{
    type Rejection = RpcError;

    async fn rpc_from_request_parts(
        _parts: &mut http::request::Parts,
        state: &OuterState,
    ) -> Result<Self, Self::Rejection> {
        let inner_state = InnerState::from_ref(state);
        Ok(Self(inner_state))
    }
}
