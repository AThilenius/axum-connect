use axum::{
    body::{self, Body},
    extract::FromRequest,
    http::{header, request, Request, StatusCode},
    response::{IntoResponse, Response},
};
use prost::Message;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::prelude::{RpcError, RpcErrorCode};

pub(crate) struct ReqResInto {
    pub streaming_req: bool,
    pub binary_res: bool,
}

pub(crate) fn encode_error(e: &RpcError, for_streaming: bool) -> Vec<u8> {
    if for_streaming {
        // See `encode_message` for the format. It's the same, except always JSON.
        let mut v = vec![0x2, 0, 0, 0, 0];
        serde_json::to_writer(&mut v, &e).unwrap();
        let size = ((v.len() - 5) as u32).to_be_bytes();
        v[1..5].copy_from_slice(&size);

        v
    } else {
        serde_json::to_vec(&e).unwrap()
    }
}

// Encode an error into a Response.
pub(crate) fn encode_error_response(
    e: &RpcError,
    as_binary: bool,
    for_streaming: bool,
) -> Response {
    if for_streaming {
        (
            // Streaming errors ALWAYS return the error in JSON, but the content type still mirrors
            // what ever the request was made with.
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                if as_binary {
                    "application/connect+proto"
                } else {
                    "application/connect+json"
                },
            )],
            encode_error(e, true),
        )
            .into_response()
    } else {
        (
            StatusCode::from(e.code.clone()),
            [(header::CONTENT_TYPE, "application/json")],
            encode_error(e, false),
        )
            .into_response()
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
            return Err(encode_error_response(
                &RpcError::new(RpcErrorCode::InvalidArgument, "Missing query".into()),
                false,
                false,
            ))
        }
    };

    let query = match serde_qs::from_str::<UnaryGetQuery>(query_str) {
        Ok(x) => x,
        Err(err) => {
            return Err(encode_error_response(
                &RpcError::new(
                    RpcErrorCode::InvalidArgument,
                    format!("Wrong query, {}", err),
                ),
                false,
                false,
            ))
        }
    };

    let binary_res = match query.encoding.as_str() {
        "json" => false,
        "proto" => true,
        s => {
            return Err(encode_error_response(
                &RpcError::new(
                    RpcErrorCode::InvalidArgument,
                    format!("Wrong or unknown query.encoding: {}", s),
                ),
                true,
                true,
            ))
        }
    };

    Ok(ReqResInto {
        binary_res,
        streaming_req: false,
    })
}

pub(crate) fn decode_check_headers(
    parts: &mut request::Parts,
    streaming_res: bool,
) -> Result<ReqResInto, Response> {
    // Check the version header, if specified.
    if let Some(version) = parts.headers.get("connect-protocol-version") {
        let version = version.to_str().unwrap_or_default();
        if version != "1" {
            return Err(encode_error_response(
                &RpcError::new(
                    RpcErrorCode::InvalidArgument,
                    format!("Unsupported protocol version: {}", version),
                ),
                true,
                streaming_res,
            ));
        }
    }

    // Decode the content type (binary or JSON).
    // TODO: I'm not sure if this is correct. The Spec doesn't say what content type will be set for
    //       server-streaming responses.
    let (binary_res, streaming_req) = match parts.headers.get("content-type") {
        Some(content_type) => match (
            content_type
                .to_str()
                .unwrap_or_default()
                .to_lowercase()
                .split(';')
                .next()
                .unwrap_or_default()
                .trim(),
            streaming_res,
        ) {
            ("application/json", false) => (false, false),
            ("application/proto", false) => (true, false),
            ("application/connect+json", true) => (false, true),
            ("application/connect+proto", true) => (true, true),
            (s, _) => {
                return Err(encode_error_response(
                    &RpcError::new(
                        RpcErrorCode::InvalidArgument,
                        format!("Wrong or unknown Content-Type: {}", s),
                    ),
                    false,
                    streaming_res,
                ))
            }
        },
        None => {
            return Err(encode_error_response(
                &RpcError::new(
                    RpcErrorCode::InvalidArgument,
                    "Missing Content-Type header".to_string(),
                ),
                false,
                streaming_res,
            ))
        }
    };

    Ok(ReqResInto {
        binary_res,
        streaming_req,
    })
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
    let streaming_res = false;

    let query_str = match parts.uri.query() {
        Some(x) => x,
        None => {
            return Err(encode_error_response(
                &RpcError::new(RpcErrorCode::InvalidArgument, "Missing query".into()),
                as_binary,
                streaming_res,
            ))
        }
    };

    let query = match serde_qs::from_str::<UnaryGetQuery>(query_str) {
        Ok(x) => x,
        Err(err) => {
            return Err(encode_error_response(
                &RpcError::new(
                    RpcErrorCode::InvalidArgument,
                    format!("Wrong query, {}", err),
                ),
                as_binary,
                streaming_res,
            ))
        }
    };

    let message = if query.base64 == Some(1) {
        use base64::{engine::general_purpose, Engine as _};

        match general_purpose::URL_SAFE.decode(&query.message) {
            Ok(x) => x,
            Err(err) => {
                return Err(encode_error_response(
                    &RpcError::new(
                        RpcErrorCode::InvalidArgument,
                        format!("Wrong query.message, {}", err),
                    ),
                    as_binary,
                    streaming_res,
                ))
            }
        }
    } else {
        query.message.as_bytes().to_vec()
    };

    if as_binary {
        let message: M = M::decode(&message[..]).map_err(|e| {
            encode_error_response(
                &RpcError::new(
                    RpcErrorCode::InvalidArgument,
                    format!("Failed to decode binary protobuf. {}", e),
                ),
                as_binary,
                streaming_res,
            )
        })?;

        Ok(message)
    } else {
        let message: M = serde_json::from_slice(&message).map_err(|e| {
            encode_error_response(
                &RpcError::new(
                    RpcErrorCode::InvalidArgument,
                    format!("Failed to decode json. {}", e),
                ),
                as_binary,
                streaming_res,
            )
        })?;

        Ok(message)
    }
}

pub(crate) async fn decode_request_payload<M, S>(
    req: Request<Body>,
    state: &S,
    binary_res: bool,
    streaming_req: bool,
) -> Result<M, Response>
where
    M: Message + DeserializeOwned + Default,
    S: Send + Sync + 'static,
{
    let bytes = body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|e| {
            encode_error_response(
                &RpcError::new(
                    RpcErrorCode::InvalidArgument,
                    format!("Failed to read request body. {}", e),
                ),
                binary_res,
                streaming_req,
            )
        })?;

    // TODO: I need an answer to https://github.com/connectrpc/connect-es/issues/1024
    // The spec doesn't seem to imply that a server-streaming response is allowed to treat the
    // request as streaming (I guess with a single message?) if content-type is set to
    // application/connect+*. That does seem to be how connect-es works though.
    let json_bytes = if streaming_req {
        // Strip and validate the envelope (the first 5 bytes).
        let mut buf = [0; 5];
    } else {
    };

    if binary_res {
        let bytes = body::to_bytes(req.into_body(), usize::MAX)
            .await
            .map_err(|e| {
                encode_error_response(
                    &RpcError::new(
                        RpcErrorCode::InvalidArgument,
                        format!("Failed to read request body. {}", e),
                    ),
                    binary_res,
                    streaming_req,
                )
            })?;

        let message: M = M::decode(bytes).map_err(|e| {
            encode_error_response(
                &RpcError::new(
                    RpcErrorCode::InvalidArgument,
                    format!("Failed to decode binary protobuf. {}", e),
                ),
                binary_res,
                streaming_req,
            )
        })?;

        Ok(message)
    } else {
        let str = match String::from_request(req, state).await {
            Ok(value) => value,
            Err(e) => {
                return Err(encode_error_response(
                    &RpcError::new(
                        RpcErrorCode::InvalidArgument,
                        format!("Failed to read request body. {}", e),
                    ),
                    binary_res,
                    streaming_req,
                ));
            }
        };

        let message: M = serde_json::from_str(&str).map_err(|e| {
            encode_error_response(
                &RpcError::new(
                    RpcErrorCode::InvalidArgument,
                    format!("Failed to decode JSON protobuf. {}", e),
                ),
                binary_res,
                streaming_req,
            )
        })?;

        Ok(message)
    }
}
