use std::{convert::Infallible, pin::Pin};

use async_stream::stream;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::{IntoResponse, Response},
};
use futures::{Future, Stream, StreamExt};
use prost::Message;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    error::RpcIntoError,
    parts::RpcFromRequestParts,
    prelude::{RpcError, RpcErrorCode},
    response::RpcIntoResponse,
};

use super::codec::{
    decode_check_headers, decode_request_payload, encode_error, encode_error_response, ReqResInto,
};

pub trait RpcHandlerStream<TMReq, TMRes, TUid, TState>: Clone + Send + Sized + 'static {
    type Future: Future<Output = Response> + Send + 'static;

    fn call(self, req: Request<Body>, state: TState) -> Self::Future;
}

// TODO: Get "connect-timeout-ms" (number as string) and apply timeout.
// TODO: Parse request metadata from:
//      - [0-9a-z]*!"-bin" ASCII value
//      - [0-9a-z]*-bin" (base64 encoded binary)
// TODO: Allow response to send back both leading and trailing metadata.
// This is here because writing Rust macros sucks a**. So I uncomment this when I'm trying to modify
// the below macro.
// #[allow(unused_parens, non_snake_case, unused_mut)]
// impl<TMReq, TMRes, TInto, TFnItem, TFnFut, TFn, TState, T1>
//     RpcHandlerStream<TMReq, TMRes, (T1, TMReq), TState> for TFn
// where
//     TMReq: Message + DeserializeOwned + Default + Send + 'static,
//     TMRes: Message + Serialize + Send + 'static,
//     TInto: RpcIntoResponse<TMRes>,
//     TFnItem: Stream<Item = TInto> + Send + Sized + 'static,
//     TFnFut: Future<Output = TFnItem> + Send + Sync,
//     TFn: FnOnce(T1, TMReq) -> TFnFut + Clone + Send + Sync + 'static,
//     TState: Send + Sync + 'static,
//     T1: RpcFromRequestParts<TMRes, TState> + Send,
// {
//     type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

//     fn call(self, req: Request<Body>, state: TState) -> Self::Future {
//         Box::pin(async move {
//             let (mut parts, body) = req.into_parts();

//             let ReqResInto { binary } = match decode_check_headers(&mut parts, true) {
//                 Ok(binary) => binary,
//                 Err(e) => return e,
//             };

//             let state = &state;

//             let t1 = match T1::rpc_from_request_parts(&mut parts, state).await {
//                 Ok(value) => value,
//                 Err(e) => {
//                     let e = e.rpc_into_error();
//                     return encode_error_response(&e, binary, true);
//                 }
//             };

//             let req = Request::from_parts(parts, body);

//             let proto_req: TMReq = match decode_request_payload(req, state, binary, true).await {
//                 Ok(value) => value,
//                 Err(e) => return e,
//             };

//             let mut res = Box::pin(self(t1, proto_req).await);

//             let res = stream! {
//                 while let Some(item) = res.next().await {
//                     let rpc_item = item.rpc_into_response();
//                     match rpc_item {
//                         Ok(rpc_item) => {
//                             if binary {
//                                 let mut res = vec![0x2, 0, 0, 0, 0];
//                                 if let Err(e) = rpc_item.encode(&mut res) {
//                                     let e = RpcError::new(RpcErrorCode::Internal, e.to_string());
//                                     yield Result::<Vec<u8>, Infallible>::Ok(encode_error(&e, true));
//                                     break;
//                                 }
//                                 let size = ((res.len() - 5) as u32).to_be_bytes();
//                                 res[1..5].copy_from_slice(&size);
//                                 yield Ok(res);
//                             } else {
//                                 let mut res = vec![0x2, 0, 0, 0, 0];
//                                 if let Err(e) = serde_json::to_writer(&mut res, &rpc_item) {
//                                     let e = RpcError::new(RpcErrorCode::Internal, e.to_string());
//                                     yield Ok(encode_error(&e, true));
//                                     break;
//                                 }
//                                 let size = ((res.len() - 5) as u32).to_be_bytes();
//                                 res[1..5].copy_from_slice(&size);
//                                 yield Ok(res);
//                             }
//                         },
//                         Err(e) => {
//                             yield Ok(encode_error(&e, binary));
//                             break;
//                         }
//                     }
//                 }

//                 // EndStreamResponse, see: https://connect.build/docs/protocol/#error-end-stream
//                 // TODO: Support returning trailers (they would need to bundle in the error type).
//                 if binary {
//                     yield Result::<Vec<u8>, Infallible>::Ok(vec![0x2, 0, 0, 0, 0]);
//                 } else {
//                     yield Result::<Vec<u8>, Infallible>::Ok(vec![0x2, 0, 0, 0, 2, b'{', b'}']);
//                 }
//             };

//             (
//                 StatusCode::OK,
//                 [(
//                     header::CONTENT_TYPE,
//                     if binary {
//                         "application/connect+proto"
//                     } else {
//                         "application/connect+json"
//                     },
//                 )],
//                 Body::from_stream(res),
//             )
//                 .into_response()
//         })
//     }
// }

macro_rules! impl_handler {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(unused_parens, non_snake_case, unused_mut)]
        impl<TMReq, TMRes, TInto, TFnItem, TFnFut, TFn, TState, $($ty,)*>
            RpcHandlerStream<TMReq, TMRes, ($($ty,)* TMReq), TState> for TFn
        where
            TMReq: Message + DeserializeOwned + Default + Send + 'static,
            TMRes: Message + Serialize + Send + 'static,
            TInto: RpcIntoResponse<TMRes>,
            TFnItem: Stream<Item = TInto> + Send + Sized + 'static,
            TFnFut: Future<Output = TFnItem> + Send + Sync,
            TFn: FnOnce($($ty,)* TMReq) -> TFnFut + Clone + Send + Sync + 'static,
            TState: Send + Sync + 'static,
            $( $ty: RpcFromRequestParts<TMRes, TState> + Send, )*
        {

            type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

            fn call(self, req: Request<Body>, state: TState) -> Self::Future {
                Box::pin(async move {
                    let (mut parts, body) = req.into_parts();

                    let ReqResInto { binary } = match decode_check_headers(&mut parts, true) {
                        Ok(binary) => binary,
                        Err(e) => return e,
                    };

                    let state = &state;

                    $(
                    let $ty = match $ty::rpc_from_request_parts(&mut parts, state).await {
                        Ok(value) => value,
                        Err(e) => {
                            let e = e.rpc_into_error();
                            return encode_error_response(&e, binary, true);
                        }
                    };
                    )*

                    let req = Request::from_parts(parts, body);

                    let proto_req: TMReq = match decode_request_payload(req, state, binary, true).await {
                        Ok(value) => value,
                        Err(e) => return e,
                    };

                    let mut res = Box::pin(self($($ty,)* proto_req).await);

                    let res = stream! {
                        while let Some(item) = res.next().await {
                            let rpc_item = item.rpc_into_response();
                            match rpc_item {
                                Ok(rpc_item) => {
                                    if binary {
                                        let mut res = vec![0x2, 0, 0, 0, 0];
                                        if let Err(e) = rpc_item.encode(&mut res) {
                                            let e = RpcError::new(RpcErrorCode::Internal, e.to_string());
                                            yield Result::<Vec<u8>, Infallible>::Ok(encode_error(&e, true));
                                            break;
                                        }
                                        let size = ((res.len() - 5) as u32).to_be_bytes();
                                        res[1..5].copy_from_slice(&size);
                                        yield Ok(res);
                                    } else {
                                        let mut res = vec![0x2, 0, 0, 0, 0];
                                        if let Err(e) = serde_json::to_writer(&mut res, &rpc_item) {
                                            let e = RpcError::new(RpcErrorCode::Internal, e.to_string());
                                            yield Ok(encode_error(&e, true));
                                            break;
                                        }
                                        let size = ((res.len() - 5) as u32).to_be_bytes();
                                        res[1..5].copy_from_slice(&size);
                                        yield Ok(res);
                                    }
                                },
                                Err(e) => {
                                    yield Ok(encode_error(&e, binary));
                                    break;
                                }
                            }
                        }

                        // EndStreamResponse, see: https://connect.build/docs/protocol/#error-end-stream
                        // TODO: Support returning trailers (they would need to bundle in the error type).
                        if binary {
                            yield Result::<Vec<u8>, Infallible>::Ok(vec![0x2, 0, 0, 0, 0]);
                        } else {
                            yield Result::<Vec<u8>, Infallible>::Ok(vec![0x2, 0, 0, 0, 2, b'{', b'}']);
                        }
                    };

                    (
                        StatusCode::OK,
                        [(
                            header::CONTENT_TYPE,
                            if binary {
                                "application/connect+proto"
                            } else {
                                "application/connect+json"
                            },
                        )],
                        Body::from_stream(res),
                    )
                        .into_response()
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
