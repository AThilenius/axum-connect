//! Protocol-generic encoding and decoding for the Connect RPC wire format.
//!
//! This module contains functions for encoding/decoding messages in both
//! JSON and binary protobuf formats, as well as envelope framing for
//! streaming RPCs.

use std::convert::Infallible;
use std::pin::Pin;

use futures::{Stream, StreamExt};
use prost::Message;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{RpcError, RpcErrorCode};

/// Result type used throughout the Connect RPC protocol.
pub type RpcResult<M> = Result<M, RpcError>;

// ── Content-type constants ──────────────────────────────────────────

/// Content-Type for unary JSON requests/responses.
pub const CONTENT_TYPE_JSON: &str = "application/json";
/// Content-Type for unary binary protobuf requests/responses.
pub const CONTENT_TYPE_PROTO: &str = "application/proto";
/// Content-Type for streaming JSON requests/responses.
pub const CONTENT_TYPE_CONNECT_JSON: &str = "application/connect+json";
/// Content-Type for streaming binary protobuf requests/responses.
pub const CONTENT_TYPE_CONNECT_PROTO: &str = "application/connect+proto";

/// Envelope flag indicating end-of-stream / trailers.
pub const ENVELOPE_FLAG_TRAILER: u8 = 0x02;

// ── Encoding ────────────────────────────────────────────────────────

type ResponseStream<M> = Pin<Box<dyn Stream<Item = RpcResult<M>> + Send>>;

/// Encode an `RpcError` as a JSON byte vector (unary error encoding).
///
/// Errors in unary calls are ALWAYS encoded as JSON.
/// <https://connectrpc.com/docs/protocol/#unary-response>
pub fn encode_unary_error(error: RpcError) -> Vec<u8> {
    serde_json::to_vec(&error).unwrap()
}

/// Encode an `RpcError` as an envelope-wrapped end-of-stream message (streaming error encoding).
///
/// Streaming errors are wrapped in `{"error": ...}` with a 5-byte envelope header.
/// <https://connectrpc.com/docs/protocol/#error-end-stream>
pub fn encode_streaming_error(error: RpcError) -> Vec<u8> {
    #[derive(Serialize)]
    struct EndOfStream {
        error: RpcError,
    }

    let message = EndOfStream { error };

    let mut result = vec![ENVELOPE_FLAG_TRAILER, 0, 0, 0, 0];
    serde_json::to_writer(&mut result, &message).unwrap();

    let size = ((result.len() - 5) as u32).to_be_bytes();
    result[1..5].copy_from_slice(&size);
    result
}

/// Encode a protobuf message as binary.
pub fn encode_unary_message_binary<M: Message>(message: M) -> Vec<u8> {
    message.encode_to_vec()
}

/// Encode a message as JSON.
pub fn encode_unary_message_json<M: Serialize>(message: M) -> RpcResult<Vec<u8>> {
    match serde_json::to_vec(&message) {
        Ok(message) => Ok(message),
        Err(error) => Err(RpcError::new(
            RpcErrorCode::Internal,
            format!("Failed to serialize response: {error}"),
        )),
    }
}

/// Encode a message into an envelope (5-byte header + payload) for streaming.
///
/// The header format is: `[flags: u8, length: u32 BE]`.
pub fn encode_envelope<M: Serialize + Message>(message: M, binary: bool) -> RpcResult<Vec<u8>> {
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

/// Wrap a stream of messages with envelope encoding plus an end-of-stream sentinel.
pub fn encode_stream<M: Serialize + Message + 'static>(
    stream: ResponseStream<M>,
    binary: bool,
) -> impl Stream<Item = Result<Vec<u8>, Infallible>> {
    futures::stream::unfold(Some(stream), move |stream| async move {
        match stream {
            None => None,
            Some(mut stream) => match stream.next().await {
                Some(Ok(message)) => match encode_envelope(message, binary) {
                    Ok(message) => Some((Ok(message), Some(stream))),
                    Err(error) => Some((Ok(encode_streaming_error(error)), None)),
                },
                Some(Err(error)) => Some((Ok(encode_streaming_error(error)), None)),
                None => {
                    // Final streaming message: empty JSON trailer.
                    // <https://connectrpc.com/docs/protocol/#error-end-stream>
                    Some((
                        Ok(vec![ENVELOPE_FLAG_TRAILER, 0, 0, 0, 2, b'{', b'}']),
                        None,
                    ))
                }
            },
        }
    })
}

// ── Decoding ────────────────────────────────────────────────────────

/// Decode a single envelope from a byte slice.
///
/// Returns `(flags, payload)` where `flags & 0x02` indicates end-of-stream.
/// The input must be at least 5 bytes (the envelope header).
pub fn decode_envelope(data: &[u8]) -> RpcResult<(u8, &[u8])> {
    if data.len() < 5 {
        return Err(RpcError::new(
            RpcErrorCode::InvalidArgument,
            "Envelope too short: need at least 5 bytes".into(),
        ));
    }

    let flags = data[0];
    let length = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;

    if data.len() < 5 + length {
        return Err(RpcError::new(
            RpcErrorCode::InvalidArgument,
            format!(
                "Envelope payload truncated: expected {} bytes, got {}",
                length,
                data.len() - 5
            ),
        ));
    }

    Ok((flags, &data[5..5 + length]))
}

/// Decode a protobuf binary message from bytes.
pub fn decode_message_binary<M: Message + Default>(data: &[u8]) -> RpcResult<M> {
    M::decode(data).map_err(|e| {
        RpcError::new(
            RpcErrorCode::InvalidArgument,
            format!("Failed to decode binary protobuf: {e}"),
        )
    })
}

/// Decode a JSON-encoded message from bytes.
pub fn decode_message_json<M: DeserializeOwned>(data: &[u8]) -> RpcResult<M> {
    serde_json::from_slice(data).map_err(|e| {
        RpcError::new(
            RpcErrorCode::InvalidArgument,
            format!("Failed to decode JSON: {e}"),
        )
    })
}

/// Decode a message from bytes, using binary or JSON encoding.
pub fn decode_message<M: Message + DeserializeOwned + Default>(
    data: &[u8],
    binary: bool,
) -> RpcResult<M> {
    if binary {
        decode_message_binary(data)
    } else {
        decode_message_json(data)
    }
}

/// Parse a Connect RPC error from a JSON response body.
pub fn decode_error(data: &[u8]) -> RpcResult<RpcError> {
    serde_json::from_slice::<RpcError>(data).map_err(|e| {
        RpcError::new(
            RpcErrorCode::Internal,
            format!("Failed to parse error response: {e}"),
        )
    })
}

/// Parse a streaming end-of-stream trailer envelope.
///
/// The trailer is a JSON object that may contain an `"error"` field.
/// Returns `Some(RpcError)` if the trailer contains an error, `None` otherwise.
pub fn decode_stream_trailer(data: &[u8]) -> Option<RpcError> {
    #[derive(serde::Deserialize)]
    struct EndOfStream {
        error: Option<RpcError>,
    }

    match serde_json::from_slice::<EndOfStream>(data) {
        Ok(eos) => eos.error,
        Err(_) => Some(RpcError::new(
            RpcErrorCode::Internal,
            "Failed to parse end-of-stream trailer".into(),
        )),
    }
}
