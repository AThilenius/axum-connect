//! Streaming response decoder for Connect RPC server-streaming responses.

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::BytesMut;
use futures::stream::Stream;
use futures::StreamExt;
use prost::Message;
use serde::de::DeserializeOwned;

use axum_connect_core::codec::{self, ENVELOPE_FLAG_TRAILER};
use axum_connect_core::error::{RpcError, RpcErrorCode};

/// A stream that decodes envelope-framed Connect RPC messages from an HTTP response body.
///
/// Each message is preceded by a 5-byte header: `[flags: u8, length: u32 BE]`.
/// When `flags & 0x02` is set, the payload is an end-of-stream trailer (JSON)
/// which may contain an error.
pub struct ConnectResponseStream<M> {
    inner: Pin<Box<dyn Stream<Item = Result<M, RpcError>> + Send>>,
}

impl<M> ConnectResponseStream<M>
where
    M: Message + DeserializeOwned + Default + 'static,
{
    pub(crate) fn new(response: reqwest::Response, binary: bool) -> Self {
        let inner = Box::pin(decode_stream::<M>(response, binary));
        ConnectResponseStream { inner }
    }
}

impl<M> Stream for ConnectResponseStream<M> {
    type Item = Result<M, RpcError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

/// Internal async generator that reads the response body stream and yields decoded messages.
fn decode_stream<M>(
    response: reqwest::Response,
    binary: bool,
) -> impl Stream<Item = Result<M, RpcError>> + Send
where
    M: Message + DeserializeOwned + Default + 'static,
{
    futures::stream::unfold(
        (response.bytes_stream(), BytesMut::new(), false),
        move |(mut body_stream, mut buf, done)| async move {
            if done {
                return None;
            }

            loop {
                // Try to parse a complete envelope from the buffer
                if buf.len() >= 5 {
                    let flags = buf[0];
                    let length =
                        u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;

                    if buf.len() >= 5 + length {
                        // We have a complete envelope
                        let _header = buf.split_to(5);
                        let payload = buf.split_to(length);

                        if flags & ENVELOPE_FLAG_TRAILER != 0 {
                            // End-of-stream trailer
                            if let Some(error) = codec::decode_stream_trailer(&payload) {
                                return Some((
                                    Err(error),
                                    (body_stream, buf, true),
                                ));
                            }
                            // Clean end of stream
                            return None;
                        }

                        // Regular message
                        let result = codec::decode_message::<M>(&payload, binary);
                        return Some((result, (body_stream, buf, false)));
                    }
                }

                // Need more data — read from the body stream
                match body_stream.next().await {
                    Some(Ok(chunk)) => {
                        buf.extend_from_slice(&chunk);
                    }
                    Some(Err(e)) => {
                        return Some((
                            Err(RpcError::new(
                                RpcErrorCode::Internal,
                                format!("Error reading response stream: {e}"),
                            )),
                            (body_stream, buf, true),
                        ));
                    }
                    None => {
                        // Stream ended without a trailer — could be a broken connection
                        if buf.is_empty() {
                            return None;
                        }
                        return Some((
                            Err(RpcError::new(
                                RpcErrorCode::Internal,
                                "Response stream ended unexpectedly without end-of-stream trailer"
                                    .into(),
                            )),
                            (body_stream, buf, true),
                        ));
                    }
                }
            }
        },
    )
}
