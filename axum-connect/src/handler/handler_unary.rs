use std::pin::Pin;

use axum::{
    body::Body,
    http::{Method, Request},
    response::Response,
};
use futures::Future;
use prost::Message;
use serde::{de::DeserializeOwned, Serialize};

use crate::{error::RpcIntoError, parts::RpcFromRequestParts, response::RpcIntoResponse};

use super::codec::{
    decode_check_headers, decode_check_query, decode_request_payload,
    decode_request_payload_from_query, ReqResInto, ResponseEncoder,
};

pub trait RpcHandlerUnary<TMReq, TMRes, TUid, TState>:
    Clone + Send + Sync + Sized + 'static
{
    type Future: Future<Output = Response> + Send + 'static;

    fn call(self, req: Request<Body>, state: TState) -> Self::Future;
}

// This is for Unary.
// TODO: Check that the header "connect-protocol-version" == "1"
// TODO: Get "connect-timeout-ms" (number as string) and apply timeout.
// TODO: Parse request metadata from:
//      - [0-9a-z]*!"-bin" ASCII value
//      - [0-9a-z]*-bin" (base64 encoded binary)
// TODO: Allow response to send back both leading and trailing metadata.

// This is here because writing Rust macros sucks a**. So I uncomment this when I'm trying to modify
// the below macro.
// #[allow(unused_parens, non_snake_case, unused_mut)]
// impl<TMReq, TMRes, TInto, TFnFut, TFn, TState, T1>
//     RpcHandlerUnary<TMReq, TMRes, (T1, TMReq), TState> for TFn
// where
//     TMReq: Message + DeserializeOwned + Default + Send + 'static,
//     TMRes: Message + Serialize + Send + 'static,
//     TInto: RpcIntoResponse<TMRes>,
//     TFnFut: Future<Output = TInto> + Send,
//     TFn: FnOnce(T1, TMReq) -> TFnFut + Clone + Send + 'static,
//     TState: Send + Sync + 'static,
//     T1: RpcFromRequestParts<TMRes, TState> + Send,
// {
//     type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

//     fn call(self, req: Request<Body>, state: TState) -> Self::Future {
//         Box::pin(async move {
//             let (mut parts, body) = req.into_parts();

//             let ReqResInto { binary } = match decode_check_headers(&mut parts, false) {
//                 Ok(binary) => binary,
//                 Err(e) => return e,
//             };

//             let state = &state;

//             let t1 = match T1::rpc_from_request_parts(&mut parts, state).await {
//                 Ok(value) => value,
//                 Err(e) => {
//                     return ResponseEncoder::empty(false, binary)
//                         .err(e.rpc_into_error())
//                         .encode_response();
//                 }
//             };

//             let proto_req: TMReq = if parts.method == Method::GET {
//                 match decode_request_payload_from_query(&parts, state, binary) {
//                     Ok(value) => value,
//                     Err(e) => return e,
//                 }
//             } else {
//                 let req = Request::from_parts(parts, body);
//
//                 match decode_request_payload(req, state, binary, false).await {
//                     Ok(value) => value,
//                     Err(e) => return e,
//                 }
//             };

//             let res = self(t1, proto_req).await.rpc_into_response();
//             match res {
//                 Ok(res) => {
//                     ResponseEncoder::<TMRes>::new(false, binary)
//                         .message(res)
//                         .encode_response()
//                 }
//                 Err(error) => {
//                     ResponseEncoder::empty(false, binary)
//                         .err(error)
//                         .encode_response()
//                 }
//             }
//         })
//     }
// }

macro_rules! impl_handler {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(unused_parens, non_snake_case, unused_mut)]
        impl<TMReq, TMRes, TInto, TFnFut, TFn, TState, $($ty,)*>
            RpcHandlerUnary<TMReq, TMRes, ($($ty,)* TMReq), TState> for TFn
        where
            TMReq: Message + DeserializeOwned + Default + Send + 'static,
            TMRes: Message + Serialize + Send + 'static,
            TInto: RpcIntoResponse<TMRes>,
            TFnFut: Future<Output = TInto> + Send,
            TFn: FnOnce($($ty,)* TMReq) -> TFnFut + Clone + Send + Sync + 'static,
            TState: Send + Sync + 'static,
            $( $ty: RpcFromRequestParts<TMRes, TState> + Send, )*
        {
            type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

            fn call(self, req: Request<Body>, state: TState) -> Self::Future {
                Box::pin(async move {
                    let (mut parts, body) = req.into_parts();

                    let ReqResInto { binary } = if parts.method == Method::GET {
                        match decode_check_query(&parts) {
                            Ok(binary) => binary,
                            Err(e) => return e,
                        }
                    } else {
                        match decode_check_headers(&mut parts, false) {
                            Ok(binary) => binary,
                            Err(e) => return e,
                        }
                    };

                    let state = &state;

                    $(
                        let $ty = match $ty::rpc_from_request_parts(&mut parts, state).await {
                            Ok(value) => value,
                            Err(e) => {
                                return ResponseEncoder::empty(false, binary)
                                    .err(e.rpc_into_error())
                                    .encode_response();
                            }
                        };
                    )*

                    let proto_req: TMReq = if parts.method == Method::GET {
                        match decode_request_payload_from_query(&parts, state, binary) {
                            Ok(value) => value,
                            Err(e) => return e,
                        }
                    } else {
                        let req = Request::from_parts(parts, body);

                        match decode_request_payload(req, state, binary, false).await {
                            Ok(value) => value,
                            Err(e) => return e,
                        }
                    };

                    let res = self($($ty,)* proto_req).await.rpc_into_response();
                    match res {
                        Ok(res) => {
                            ResponseEncoder::<TMRes>::new(false, binary)
                                .message(res)
                                .encode_response()
                        }
                        Err(error) => {
                            ResponseEncoder::empty(false, binary)
                                .err(error)
                                .encode_response()
                        }
                    }
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
