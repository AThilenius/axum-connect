use std::pin::Pin;

use async_stream::stream;
use axum::{body::Body, http::Request, response::Response};
use futures::{Future, Stream, StreamExt};
use prost::Message;
use serde::{de::DeserializeOwned, Serialize};

use crate::{error::RpcIntoError, parts::RpcFromRequestParts, response::RpcIntoResponse};

use super::codec::{decode_check_headers, decode_request_payload, ReqResInto, ResponseEncoder};

pub trait RpcHandlerStream<TMReq, TMRes, TUid, TState>:
    Clone + Send + Sync + Sized + 'static
{
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
//                     return ResponseEncoder::empty(true, binary)
//                         .err(e.rpc_into_error())
//                         .encode_response();
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
//                     yield Ok(item.rpc_into_response()?);
//                 }
//             };
//
//             // EndStreamResponse, see: https://connect.build/docs/protocol/#error-end-stream
//             // TODO: Support returning trailers (they would need to bundle in the error type).
//             ResponseEncoder::<TMRes>::new(true, binary)
//                 .stream(res)
//                 .encode_response()
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
                            return ResponseEncoder::empty(true, binary)
                                .err(e.rpc_into_error())
                                .encode_response();
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
                            yield Ok(item.rpc_into_response()?);
                        }
                    };

                    // TODO: Support returning trailers (they would need to bundle in the error type).
                    ResponseEncoder::<TMRes>::new(true, binary)
                        .stream(res)
                        .encode_response()
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
