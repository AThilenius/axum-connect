use std::convert::Infallible;
use std::pin::Pin;

use axum::{
    body::{self, Body},
    http::{header, request, Request, StatusCode},
    response::{IntoResponse, Response},
};
use futures::{Stream, StreamExt};
use prost::Message;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::prelude::{RpcError, RpcErrorCode, RpcResult};

pub(crate) struct ReqResInto {
    pub binary: bool,
}

pub(crate) struct ResponseEncoder<M> {
    streaming: bool,
    binary: bool,
    error: Option<RpcError>,
    message: Option<M>,
    stream: Option<Pin<Box<dyn Stream<Item = RpcResult<M>> + Send>>>,
}

impl ResponseEncoder<()> {
    pub fn empty(streaming: bool, binary: bool) -> Self {
        Self::new(streaming, binary)
    }
}

impl<M: Message + Serialize + 'static> ResponseEncoder<M> {
    pub fn new(streaming: bool, binary: bool) -> Self {
        Self {
            streaming,
            binary,
            error: None,
            message: None,
            stream: None,
        }
    }

    pub fn err(self, error: RpcError) -> Self {
        self.err_opt(Some(error))
    }

    pub fn err_opt(mut self, error: Option<RpcError>) -> Self {
        self.error = error;
        self
    }

    pub fn stream(mut self, stream: impl Stream<Item = RpcResult<M>> + Send + 'static) -> Self {
        self.stream = Some(stream.boxed());
        self
    }

    pub fn message(mut self, message: M) -> Self {
        self.message = Some(message);
        self
    }

    pub fn status_code(&self) -> StatusCode {
        // Streaming requests ALWAYS return 200 response code
        // https://connectrpc.com/docs/protocol/#streaming-response
        if self.streaming {
            return StatusCode::OK;
        }

        self.error
            .as_ref()
            .map(|e| e.code.clone())
            .map(StatusCode::from)
            .unwrap_or(StatusCode::OK)
    }

    pub fn content_type(&self) -> &'static str {
        match (self.streaming, self.binary, self.error.as_ref()) {
            // Streaming
            (true, false, _) => "application/connect+json",
            (true, true, _) => "application/connect+proto",

            // Errors in unary calls are ALWAYS encoded as JSONs
            // https://connectrpc.com/docs/protocol/#unary-response
            (false, _, Some(_)) => "application/json",

            // Unary successful
            (false, false, None) => "application/json",
            (false, true, None) => "application/proto",
        }
    }

    fn encode_message_enveloped(&mut self, message: &M) -> (Vec<u8>, Option<&RpcError>) {
        let mut result = vec![0, 0, 0, 0, 0];

        if self.binary {
            if let Err(error) = message.encode(&mut result) {
                self.error = Some(RpcError::new(RpcErrorCode::Internal, error.to_string()));
                return (self.encode_message_end(), self.error.as_ref());
            }
        } else if let Err(error) = serde_json::to_writer(&mut result, &message) {
            self.error = Some(RpcError::new(RpcErrorCode::Internal, error.to_string()));
            return (self.encode_message_end(), self.error.as_ref());
        }

        let size = ((result.len() - 5) as u32).to_be_bytes();
        result[1..5].copy_from_slice(&size);
        (result, None)
    }

    fn encode_message_unary(&mut self, message: &M) -> (Vec<u8>, Option<&RpcError>) {
        if self.binary {
            (message.encode_to_vec(), None)
        } else {
            match serde_json::to_vec(&message) {
                Ok(message) => (message, None),
                Err(error) => {
                    self.error = Some(RpcError::new(
                        RpcErrorCode::Internal,
                        format!("Failed to serialize response: {error}"),
                    ));

                    (self.encode_message_end(), self.error.as_ref())
                }
            }
        }
    }

    fn encode_message_end(&mut self) -> Vec<u8> {
        if let Some(error) = self.error.as_ref() {
            // Errors in unary calls are ALWAYS encoded as JSONs
            //
            // Streaming errors are wrapped in an { "error": ... }
            // while unary errors are just plain JSON encoded.
            //
            // https://connectrpc.com/docs/protocol/#unary-response
            // https://connectrpc.com/docs/protocol/#error-end-stream
            if self.streaming {
                #[derive(Serialize)]
                struct EndOfStream<'a> {
                    error: &'a RpcError,
                }

                let message = EndOfStream { error };

                let mut result = vec![0x2, 0, 0, 0, 0];
                serde_json::to_writer(&mut result, &message).unwrap();

                let size = ((result.len() - 5) as u32).to_be_bytes();
                result[1..5].copy_from_slice(&size);

                result
            } else {
                serde_json::to_vec(&error).unwrap()
            }
        } else if self.streaming {
            // Final streaming message ALWAYS has to contain at least
            // an empty object and is ALWAYS encoded as JSON.
            // https://connectrpc.com/docs/protocol/#error-end-stream
            vec![0x2, 0, 0, 0, 2, b'{', b'}']
        } else {
            Vec::new()
        }
    }

    fn encode_body_streaming(mut self) -> Body {
        if self.error.is_some() {
            // Error was set outside of the stream.
            // This is most likely fatal and we probably
            // should not stream stuff but just return the error.
            Body::from(self.encode_message_end())
        } else if let Some(stream) = self.stream.take() {
            // Streaming call with a stream set
            //
            // This was born in hell and in hell it shall stay.
            // For mortals, it simply ensures that all messages
            // inside the stream are passed along envelope-encodeed
            // and upon reaching the end, the end message is added.
            //
            // At this this stage the only errors can come from within
            // the stream and this thing handles that case by simply
            // encoding the error end terminating the stream.
            Body::from_stream(futures::stream::unfold(
                (stream, Some(self)),
                move |(mut stream, this)| async move {
                    match (stream.next().await, this) {
                        (_, None) => {
                            // The `this` was not set, which means we are past
                            // the last message, returning None ends the stream
                            // without any more messages.
                            //
                            // The type annotation is simply to tell the complier
                            // there are no `Err`s in the result stream since
                            // `Body::from_stream` requires a impl TryStream.
                            Option::<(Result<Vec<u8>, Infallible>, _)>::None
                        }
                        (Some(Ok(message)), Some(mut this)) => {
                            // This is a normal message, we need to envelope-encode it.
                            // If an error occurs, it will be emitted instead of
                            // the original message, so we just have to make sure
                            // to terminate the stream.
                            match this.encode_message_enveloped(&message) {
                                (message, None) => Some((Ok(message), (stream, Some(this)))),
                                (message, Some(_)) => Some((Ok(message), (stream, None))),
                            }
                        }
                        (Some(Err(error)), Some(this)) => {
                            // An error in the stream. Send it as the last
                            // message and terminate the stream.
                            let message = this.err(error).encode_message_end();
                            Some((Ok(message), (stream, None)))
                        }
                        (None, Some(mut this)) => {
                            // Stream was read all the way through without errors,
                            // send the last message.
                            let message = this.encode_message_end();
                            Some((Ok(message), (stream, None)))
                        }
                    }
                },
            ))
        } else {
            // A streaming message without a stream present.
            // This is technically a valid state, just send
            // the ending message.
            Body::from(self.encode_message_end())
        }
    }

    fn encode_body_unary(mut self) -> Body {
        if let Some(message) = self.message.take() {
            Body::from(self.encode_message_unary(&message).0)
        } else {
            Body::from(self.encode_message_end())
        }
    }

    fn encode_body(self) -> Body {
        if self.streaming {
            self.encode_body_streaming()
        } else {
            self.encode_body_unary()
        }
    }

    pub fn encode_response(self) -> Response {
        let code = self.status_code();
        let headers = [(header::CONTENT_TYPE, self.content_type())];
        let body = self.encode_body();
        (code, headers, body).into_response()
    }
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
            return Err(ResponseEncoder::empty(false, false)
                .err(error)
                .encode_response());
        }
    };

    let query = match serde_qs::from_str::<UnaryGetQuery>(query_str) {
        Ok(x) => x,
        Err(err) => {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Wrong query, {}", err),
            );

            return Err(ResponseEncoder::empty(false, false)
                .err(error)
                .encode_response());
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

            return Err(ResponseEncoder::empty(true, true)
                .err(error)
                .encode_response());
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

            return Err(ResponseEncoder::empty(for_streaming, true)
                .err(error)
                .encode_response());
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

                return Err(ResponseEncoder::empty(true, true)
                    .err(error)
                    .encode_response());
            }
        },
        None => {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                "Missing Content-Type header".to_string(),
            );

            return Err(ResponseEncoder::empty(true, true)
                .err(error)
                .encode_response());
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
            return Err(ResponseEncoder::empty(false, false)
                .err(error)
                .encode_response());
        }
    };

    let query = match serde_qs::from_str::<UnaryGetQuery>(query_str) {
        Ok(x) => x,
        Err(err) => {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Wrong query, {}", err),
            );

            return Err(ResponseEncoder::empty(false, false)
                .err(error)
                .encode_response());
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

                return Err(ResponseEncoder::empty(false, false)
                    .err(error)
                    .encode_response());
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

            ResponseEncoder::empty(for_streaming, as_binary)
                .err(error)
                .encode_response()
        })?;

        Ok(message)
    } else {
        let message: M = serde_json::from_slice(&message).map_err(|e| {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Failed to decode json. {}", e),
            );

            ResponseEncoder::empty(for_streaming, as_binary)
                .err(error)
                .encode_response()
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

            ResponseEncoder::empty(for_streaming, as_binary)
                .err(error)
                .encode_response()
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

            ResponseEncoder::empty(for_streaming, as_binary)
                .err(error)
                .encode_response()
        })?;

        Ok(message)
    } else {
        let message: M = serde_json::from_slice(&bytes).map_err(|e| {
            let error = RpcError::new(
                RpcErrorCode::InvalidArgument,
                format!("Failed to decode JSON protobuf. {}", e),
            );

            ResponseEncoder::empty(for_streaming, as_binary)
                .err(error)
                .encode_response()
        })?;

        Ok(message)
    }
}
