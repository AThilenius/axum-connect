use std::convert::Infallible;
use std::pin::Pin;

use axum::body::{self, Body};
use axum::http::{header, request, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use futures::{Stream, StreamExt};
use prost::Message;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{RpcError, RpcErrorCode, RpcIntoError};
use crate::response::{RpcIntoResponse, RpcResult};

pub(crate) struct ReqResInto {
    pub binary: bool,
}

type ResponseStream<M> = Pin<Box<dyn Stream<Item = RpcResult<M>> + Send>>;

enum ResponseContent<M> {
    UnarySuccess(M),
    UnaryError(RpcError),
    StreamingSuccess(ResponseStream<M>),
    StreamingError(RpcError),
}

pub(crate) struct ResponseEncoder<M> {
    binary: bool,
    content: ResponseContent<M>,
}

impl ResponseEncoder<()> {
    pub fn error(error: impl RpcIntoError, streaming: bool, binary: bool) -> Self {
        Self {
            binary,
            content: if streaming {
                ResponseContent::StreamingError(error.rpc_into_error())
            } else {
                ResponseContent::UnaryError(error.rpc_into_error())
            },
        }
    }
}

impl<M: Message + Serialize + 'static> ResponseEncoder<M> {
    pub fn unary(response: impl RpcIntoResponse<M>, binary: bool) -> Self {
        Self {
            binary,
            content: match response.rpc_into_response() {
                Ok(message) => ResponseContent::UnarySuccess(message),
                Err(error) => ResponseContent::UnaryError(error),
            },
        }
    }

    pub fn stream(stream: ResponseStream<M>, binary: bool) -> Self {
        Self {
            binary,
            content: ResponseContent::StreamingSuccess(stream),
        }
    }

    pub fn status_code(&self) -> StatusCode {
        use ResponseContent::*;

        match &self.content {
            UnarySuccess(_) => StatusCode::OK,
            UnaryError(e) => e.code.clone().into(),

            // Streaming requests ALWAYS return 200 response code
            // https://connectrpc.com/docs/protocol/#streaming-response
            StreamingSuccess(_) | StreamingError(_) => StatusCode::OK,
        }
    }

    pub fn content_type(&self) -> &'static str {
        use ResponseContent::*;

        match (&self.content, self.binary) {
            // Streaming
            (StreamingSuccess(_) | StreamingError(_), false) => "application/connect+json",
            (StreamingSuccess(_) | StreamingError(_), true) => "application/connect+proto",

            // Errors in unary calls are ALWAYS encoded as JSONs
            // https://connectrpc.com/docs/protocol/#unary-response
            (UnaryError(_), _) => "application/json",

            // Unary successful
            (UnarySuccess(_), false) => "application/json",
            (UnarySuccess(_), true) => "application/proto",
        }
    }

    fn encode_body(self) -> Body {
        use ResponseContent::*;

        match self.content {
            // Error
            UnaryError(error) => Body::from(encode_unary_error(error)),
            StreamingError(error) => Body::from(encode_streaming_error(error)),

            // Unary
            UnarySuccess(message) => Body::from(if self.binary {
                encode_unary_message_binary(message)
            } else {
                encode_unary_message_json(message).unwrap_or_else(encode_unary_error)
            }),

            // Streaming
            StreamingSuccess(stream) => Body::from_stream(encode_stream(stream, self.binary)),
        }
    }

    pub fn encode_response(self) -> Response {
        let code = self.status_code();
        let headers = [(header::CONTENT_TYPE, self.content_type())];
        let body = self.encode_body();
        (code, headers, body).into_response()
    }
}

fn encode_unary_error(error: RpcError) -> Vec<u8> {
    // Errors in unary calls are ALWAYS encoded as JSONs
    //
    // https://connectrpc.com/docs/protocol/#unary-response
    serde_json::to_vec(&error).unwrap()
}

fn encode_streaming_error(error: RpcError) -> Vec<u8> {
    // Streaming errors are wrapped in an { "error": ... }
    // while unary errors are just plain JSON encoded.
    //
    // https://connectrpc.com/docs/protocol/#error-end-stream
    #[derive(Serialize)]
    struct EndOfStream {
        error: RpcError,
    }

    let message = EndOfStream { error };

    let mut result = vec![0x2, 0, 0, 0, 0];
    serde_json::to_writer(&mut result, &message).unwrap();

    let size = ((result.len() - 5) as u32).to_be_bytes();
    result[1..5].copy_from_slice(&size);
    result
}

fn encode_unary_message_binary<M: Message>(message: M) -> Vec<u8> {
    message.encode_to_vec()
}

fn encode_unary_message_json<M: Serialize>(message: M) -> RpcResult<Vec<u8>> {
    match serde_json::to_vec(&message) {
        Ok(message) => Ok(message),
        Err(error) => Err(RpcError::new(
            RpcErrorCode::Internal,
            format!("Failed to serialize response: {error}"),
        )),
    }
}

fn encode_envelope<M: Serialize + Message>(message: M, binary: bool) -> RpcResult<Vec<u8>> {
    let mut result = vec![0, 0, 0, 0, 0];

    if binary {
        if let Err(error) = message.encode(&mut result) {
            return Err(RpcError::new(RpcErrorCode::Internal, error.to_string()));
        }
    } else if let Err(error) = serde_json::to_writer(&mut result, &message) {
        return Err(RpcError::new(RpcErrorCode::Internal, error.to_string()));
    }

    let size = ((result.len() - 5) as u32).to_be_bytes();
    result[1..5].copy_from_slice(&size);
    Ok(result)
}

fn encode_stream<M: Serialize + Message + 'static>(
    stream: ResponseStream<M>,
    binary: bool,
) -> impl Stream<Item = Result<Vec<u8>, Infallible>> {
    // This was born in hell and in hell it shall stay.
    // For mortals, it simply ensures that all messages
    // inside the stream are passed along envelope-encodeed
    // and upon reaching the end, the end message is added.
    //
    // At this this stage the only errors can come from within
    // the stream and this thing handles that case by simply
    // encoding the error end terminating the stream.
    futures::stream::unfold(Some(stream), move |stream| async move {
        match stream {
            None => {
                // We are past the last message, returning None
                // ends the stream without any more messages.
                None
            }
            Some(mut stream) => match stream.next().await {
                Some(Ok(message)) => {
                    // This is a normal message, we need to envelope-encode it.
                    // If an error occurs, we encode it instead and terminate
                    // the stream.
                    match encode_envelope(message, binary) {
                        Ok(message) => Some((Ok(message), Some(stream))),
                        Err(error) => Some((Ok(encode_streaming_error(error)), None)),
                    }
                }
                Some(Err(error)) => {
                    // An error in the stream. Send it as the last
                    // message and terminate the stream.
                    Some((Ok(encode_streaming_error(error)), None))
                }
                None => {
                    // Stream was read all the way through without errors,
                    // send the last message.
                    //
                    // Final streaming message ALWAYS has to contain at least
                    // an empty object and is ALWAYS encoded as JSON.
                    // https://connectrpc.com/docs/protocol/#error-end-stream
                    Some((Ok(vec![0x2, 0, 0, 0, 2, b'{', b'}']), None))
                }
            },
        }
    })
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct UnaryGetQuery {
    pub message: String,
    pub encoding: String,
    pub base64: Option<usize>,
    pub compression: Option<String>,
    pub connect: Option<String>,
}

pub(crate) fn decode_check_query(parts: &request::Parts) -> Result<ReqResInto, Response> {
    let query_str = match parts.uri.query() {
        Some(x) => x,
        None => {
            let error = RpcError::new(RpcErrorCode::InvalidArgument, "Missing query".into());
            return Err(ResponseEncoder::error(error, false, false).encode_response());
        }
    };

    let query = match serde_qs::from_str::<UnaryGetQuery>(query_str) {
        Ok(x) => x,
        Err(err) => {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Wrong query, {}", err),
            );

            return Err(ResponseEncoder::error(error, false, false).encode_response());
        }
    };

    let binary = match query.encoding.as_str() {
        "json" => false,
        "proto" => true,
        s => {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Wrong or unknown query.encoding: {}", s),
            );

            return Err(ResponseEncoder::error(error, true, true).encode_response());
        }
    };

    Ok(ReqResInto { binary })
}

pub(crate) fn decode_check_headers(
    parts: &mut request::Parts,
    for_streaming: bool,
) -> Result<ReqResInto, Response> {
    // Check the version header, if specified.
    if let Some(version) = parts.headers.get("connect-protocol-version") {
        let version = version.to_str().unwrap_or_default();
        if version != "1" {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Unsupported protocol version: {}", version),
            );

            return Err(ResponseEncoder::error(error, for_streaming, true).encode_response());
        }
    }

    // Decode the content type (binary or JSON).
    // TODO: I'm not sure if this is correct. The Spec doesn't say what content type will be set for
    //       server-streaming responses.
    let binary = match parts.headers.get("content-type") {
        Some(content_type) => match (
            content_type
                .to_str()
                .unwrap_or_default()
                .to_lowercase()
                .split(';')
                .next()
                .unwrap_or_default()
                .trim(),
            for_streaming,
        ) {
            ("application/json", false) => false,
            ("application/proto", false) => true,
            ("application/connect+json", true) => false,
            ("application/connect+proto", true) => true,
            (s, _) => {
                let error = RpcError::new(
                    RpcErrorCode::InvalidArgument,
                    format!("Wrong or unknown Content-Type: {}", s),
                );

                return Err(ResponseEncoder::error(error, true, true).encode_response());
            }
        },
        None => {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                "Missing Content-Type header".to_string(),
            );

            return Err(ResponseEncoder::error(error, true, true).encode_response());
        }
    };

    Ok(ReqResInto { binary })
}

pub(crate) fn decode_request_payload_from_query<M, S>(
    parts: &request::Parts,
    _state: &S,
    as_binary: bool,
) -> Result<M, Response>
where
    M: Message + DeserializeOwned + Default,
    S: Send + Sync + 'static,
{
    let for_streaming = false;

    let query_str = match parts.uri.query() {
        Some(x) => x,
        None => {
            let error = RpcError::new(RpcErrorCode::InvalidArgument, "Missing query".to_string());
            return Err(ResponseEncoder::error(error, false, false).encode_response());
        }
    };

    let query = match serde_qs::from_str::<UnaryGetQuery>(query_str) {
        Ok(x) => x,
        Err(err) => {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Wrong query, {}", err),
            );

            return Err(ResponseEncoder::error(error, false, false).encode_response());
        }
    };

    let message = if query.base64 == Some(1) {
        use base64::{engine::general_purpose, Engine as _};

        match general_purpose::URL_SAFE.decode(&query.message) {
            Ok(x) => x,
            Err(err) => {
                let error = RpcError::new(
                    RpcErrorCode::InvalidArgument,
                    format!("Wrong query.message, {}", err),
                );

                return Err(ResponseEncoder::error(error, false, false).encode_response());
            }
        }
    } else {
        query.message.as_bytes().to_vec()
    };

    if as_binary {
        let message: M = M::decode(&message[..]).map_err(|e| {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Failed to decode binary protobuf. {}", e),
            );

            ResponseEncoder::error(error, for_streaming, as_binary).encode_response()
        })?;

        Ok(message)
    } else {
        let message: M = serde_json::from_slice(&message).map_err(|e| {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Failed to decode json. {}", e),
            );

            ResponseEncoder::error(error, for_streaming, as_binary).encode_response()
        })?;

        Ok(message)
    }
}

pub(crate) async fn decode_request_payload<M, S>(
    req: Request<Body>,
    _state: &S,
    as_binary: bool,
    for_streaming: bool,
) -> Result<M, Response>
where
    M: Message + DeserializeOwned + Default,
    S: Send + Sync + 'static,
{
    let bytes = body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|e| {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Failed to read request body. {}", e),
            );

            ResponseEncoder::error(error, for_streaming, as_binary).encode_response()
        })?;

    // All streaming messages are wrapped in an envelope,
    // even if they are just requests for server-streaming.
    // https://connectrpc.com/docs/protocol/#streaming-request
    // https://github.com/connectrpc/connectrpc.com/issues/141
    // TODO: Parse the envelope (containing flags u8 and length u32)
    let bytes = bytes.slice(if for_streaming { 5.. } else { 0.. });

    if as_binary {
        let message: M = M::decode(bytes).map_err(|e| {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Failed to decode binary protobuf. {}", e),
            );

            ResponseEncoder::error(error, for_streaming, as_binary).encode_response()
        })?;

        Ok(message)
    } else {
        let message: M = serde_json::from_slice(&bytes).map_err(|e| {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Failed to decode JSON protobuf. {}", e),
            );

            ResponseEncoder::error(error, for_streaming, as_binary).encode_response()
        })?;

        Ok(message)
    }
}
