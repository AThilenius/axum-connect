use std::pin::Pin;

use axum::{
    body::{Body, HttpBody},
    extract::{FromRequest, FromRequestParts},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    BoxError, Router,
};
use futures::Future;
use protobuf::MessageFull;
use serde::Serialize;

pub use protobuf;
pub use protobuf_json_mapping;

pub trait RpcRouterExt<S, B>: Sized {
    fn rpc<F>(self, register: F) -> Self
    where
        F: FnOnce(Self) -> RpcRouter<S, B>;
}

impl<S, B> RpcRouterExt<S, B> for Router<S, B> {
    fn rpc<F>(self, register: F) -> Self
    where
        F: FnOnce(Self) -> RpcRouter<S, B>,
    {
        register(self)
    }
}

pub type RpcRouter<S, B> = Router<S, B>;

pub trait RegisterRpcService<S, B>: Sized {
    fn register(self, router: Router<S, B>) -> Self;
}

pub trait IntoRpcResponse<T>
where
    T: MessageFull,
{
    fn into_response(self) -> Response;
}

#[derive(Clone, Serialize)]
pub struct RpcError {
    pub code: RpcErrorCode,
    pub message: String,
    pub details: Vec<RpcErrorDetail>,
}

impl RpcError {
    pub fn new(code: RpcErrorCode, message: String) -> Self {
        Self {
            code,
            message,
            details: vec![],
        }
    }
}

#[derive(Clone, Serialize)]
pub struct RpcErrorDetail {
    #[serde(rename = "type")]
    pub proto_type: String,
    #[serde(rename = "value")]
    pub proto_b62_value: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RpcErrorCode {
    Canceled,
    Unknown,
    InvalidArgument,
    DeadlineExceeded,
    NotFound,
    AlreadyExists,
    PermissionDenied,
    ResourceExhausted,
    FailedPrecondition,
    Aborted,
    OutOfRange,
    Unimplemented,
    Internal,
    Unavailable,
    DataLoss,
    Unauthenticated,
}

impl From<RpcErrorCode> for StatusCode {
    fn from(val: RpcErrorCode) -> Self {
        match val {
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
}

impl IntoResponse for RpcError {
    fn into_response(self) -> Response {
        let status_code = StatusCode::from(self.code.clone());
        let json = serde_json::to_string(&self).expect("serialize error type");
        (status_code, json).into_response()
    }
}

impl<T, E> IntoRpcResponse<T> for Result<T, E>
where
    T: MessageFull,
    E: Into<RpcError>,
{
    fn into_response(self) -> Response {
        match self {
            Ok(res) => rpc_to_response(res),
            Err(err) => err.into().into_response(),
        }
    }
}

pub trait HandlerFuture<TReq, TRes, T, S, B = Body>: Clone + Send + Sized + 'static {
    type Future: Future<Output = TRes> + Send + 'static;

    fn call(self, req: Request<B>, state: S) -> Self::Future;
}

fn rpc_to_response<T>(res: T) -> Response
where
    T: MessageFull,
{
    protobuf_json_mapping::print_to_string(&res)
        .map_err(|_e| {
            RpcError::new(
                RpcErrorCode::Internal,
                "Failed to serialize response".to_string(),
            )
        })
        .into_response()
}

macro_rules! impl_handler {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(unused_parens, non_snake_case, unused_mut)]
        impl<TReq, TRes, F, Fut, S, B, $($ty,)*> HandlerFuture<TReq, TRes, ($($ty,)* TReq), S, B> for F
        where
            TReq: MessageFull + Send + 'static,
            TRes: MessageFull + Send + 'static,
            F: FnOnce($($ty,)* TReq) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = TRes> + Send,
            B: HttpBody + Send + 'static,
            B::Data: Send,
            B::Error: Into<BoxError>,
            S: Send + Sync + 'static,
            $( $ty: FromRequestParts<S> + Send, )*
        {
            type Future = Pin<Box<dyn Future<Output = TRes> + Send>>;

            fn call(self, req: Request<B>, state: S) -> Self::Future {
                Box::pin(async move {
                    let (mut parts, body) = req.into_parts();
                    let state = &state;

                    // This would be done by macro expansion. It also wouldn't be unwrapped, but
                    // there is no error union so I can't return a rejection.
                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, state).await {
                            Ok(value) => value,
                            Err(_e) => unreachable!(),
                        };
                    )*

                    let req = Request::from_parts(parts, body);

                    let body = match String::from_request(req, state).await {
                        Ok(value) => value,
                        Err(_e) => unreachable!(),
                    };

                    let proto_req: TReq = match protobuf_json_mapping::parse_from_str(&body) {
                        Ok(value) => value,
                        Err(_e) => unreachable!(),
                    };

                    let res = self($($ty,)* proto_req).await;
                    res
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
