use std::pin::Pin;

use axum::{body::HttpBody, extract::FromRequest, http::Request, BoxError};
use futures::Future;
use protobuf::MessageFull;

pub use protobuf;
pub use protobuf_json_mapping;

pub use crate::{error::RpcIntoError, parts::RpcFromRequestParts, response::RpcIntoResponse};
use crate::{
    error::{RpcError, RpcErrorCode},
    prelude::RpcResponse,
};

pub trait HandlerFuture<TReq, TRes, Res, T, S, B>: Clone + Send + Sized + 'static {
    type Future: Future<Output = RpcResponse<TRes>> + Send + 'static;

    fn call(self, req: Request<B>, state: S) -> Self::Future;
}

// This is a single expanded version of the macro below. It's left here for ease of reading and
// understanding the macro, as well as development.
// ```rust
// #[allow(unused_parens, non_snake_case, unused_mut)]
// impl<TReq, TRes, Res, F, Fut, S, B, T1> HandlerFuture<TReq, TRes, Res, (T1, TReq), S, B> for F
// where
//     TReq: MessageFull + Send + 'static,
//     TRes: MessageFull + Send + 'static,
//     Res: RpcIntoResponse<TRes>,
//     F: FnOnce(T1, TReq) -> Fut + Clone + Send + 'static,
//     Fut: Future<Output = Res> + Send,
//     B: HttpBody + Send + 'static,
//     B::Data: Send,
//     B::Error: Into<BoxError>,
//     S: Send + Sync + 'static,
//     T1: RpcFromRequestParts<TRes, S> + Send,
// {
//     type Future = Pin<Box<dyn Future<Output = RpcResponse<TRes>> + Send>>;

//     fn call(self, req: Request<B>, state: S) -> Self::Future {
//         Box::pin(async move {
//             let (mut parts, body) = req.into_parts();
//             let state = &state;

//             let t1 = match T1::rpc_from_request_parts(&mut parts, state).await {
//                 Ok(value) => value,
//                 Err(e) => return e.rpc_into_error().rpc_into_response(),
//             };

//             let req = Request::from_parts(parts, body);

//             let body = match String::from_request(req, state).await {
//                 Ok(value) => value,
//                 Err(e) => {
//                     return RpcError::new(RpcErrorCode::FailedPrecondition, e.to_string())
//                         .rpc_into_response()
//                 }
//             };

//             let proto_req: TReq = match protobuf_json_mapping::parse_from_str(&body) {
//                 Ok(value) => value,
//                 Err(_e) => {
//                     return RpcError::new(
//                         RpcErrorCode::InvalidArgument,
//                         "Failed to parse request".to_string(),
//                     )
//                     .rpc_into_response()
//                 }
//             };

//             let res = self(t1, proto_req).await;

//             res.rpc_into_response()
//         })
//     }
// }
// ```
macro_rules! impl_handler {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(unused_parens, non_snake_case, unused_mut)]
        impl<TReq, TRes, Res, F, Fut, S, B, $($ty,)*>
            HandlerFuture<TReq, TRes, Res, ($($ty,)* TReq), S, B> for F
        where
            TReq: MessageFull + Send + 'static,
            TRes: MessageFull + Send + 'static,
            Res: RpcIntoResponse<TRes>,
            F: FnOnce($($ty,)* TReq) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = Res> + Send,
            B: HttpBody + Send + 'static,
            B::Data: Send,
            B::Error: Into<BoxError>,
            S: Send + Sync + 'static,
            $( $ty: RpcFromRequestParts<TRes, S> + Send, )*
        {
            type Future = Pin<Box<dyn Future<Output = RpcResponse<TRes>> + Send>>;

            fn call(self, req: Request<B>, state: S) -> Self::Future {
                Box::pin(async move {
                    let (mut parts, body) = req.into_parts();
                    let state = &state;

                    $(
                        let $ty = match $ty::rpc_from_request_parts(&mut parts, state).await {
                            Ok(value) => value,
                            Err(e) => return e.rpc_into_error().rpc_into_response(),
                        };
                    )*

                    let req = Request::from_parts(parts, body);

                    let body = match String::from_request(req, state).await {
                        Ok(value) => value,
                        Err(e) => {
                            return RpcError::new(RpcErrorCode::FailedPrecondition, e.to_string())
                                .rpc_into_response()
                        }
                    };

                    let proto_req: TReq = match protobuf_json_mapping::parse_from_str(&body) {
                        Ok(value) => value,
                        Err(_e) => {
                            return RpcError::new(
                                RpcErrorCode::InvalidArgument,
                                "Failed to parse request".to_string(),
                            )
                            .rpc_into_response()
                        }
                    };

                    let res = self($($ty,)* proto_req).await;

                    res.rpc_into_response()
                })
            }
        }
    };
}

impl_handler!([]);
impl_handler!([T1]);
impl_handler!([T1, T2]);
impl_handler!([T1, T2, T3]);
impl_handler!([T1, T2, T3, T4]);
impl_handler!([T1, T2, T3, T4, T5]);
impl_handler!([T1, T2, T3, T4, T5, T6]);
impl_handler!([T1, T2, T3, T4, T5, T6, T7]);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8]);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9]);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10]);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11]);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12]);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13]);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14]);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15]);
