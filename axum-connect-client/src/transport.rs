//! Connect RPC HTTP transport using `reqwest`.

use prost::Message;
use reqwest::header::CONTENT_TYPE;
use serde::de::DeserializeOwned;
use serde::Serialize;

use axum_connect_core::codec::{
    self, RpcResult, CONTENT_TYPE_CONNECT_JSON, CONTENT_TYPE_CONNECT_PROTO, CONTENT_TYPE_JSON,
    CONTENT_TYPE_PROTO,
};
use axum_connect_core::error::{RpcError, RpcErrorCode};

use crate::stream::ConnectResponseStream;

/// A Connect RPC client that sends requests to a remote server.
///
/// The client is cheap to clone (it shares the underlying `reqwest::Client`
/// and configuration). Generated service client structs hold a clone of this.
#[derive(Clone, Debug)]
pub struct ConnectClient {
    base_url: String,
    client: reqwest::Client,
    use_binary: bool,
}

/// Builder for configuring a `ConnectClient`.
pub struct ConnectClientBuilder {
    base_url: String,
    client: Option<reqwest::Client>,
    use_binary: bool,
}

impl ConnectClient {
    /// Create a new client with the given base URL (e.g. `"http://localhost:3030"`).
    ///
    /// Uses JSON encoding by default.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            use_binary: false,
        }
    }

    /// Create a builder for more advanced configuration.
    pub fn builder(base_url: impl Into<String>) -> ConnectClientBuilder {
        ConnectClientBuilder {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client: None,
            use_binary: false,
        }
    }

    /// Send a unary (request-response) RPC call.
    ///
    /// This is used by generated client stubs — you normally don't call it directly.
    pub async fn unary<Req, Res>(&self, path: &str, request: Req) -> RpcResult<Res>
    where
        Req: Message + Serialize,
        Res: Message + DeserializeOwned + Default,
    {
        let url = format!("{}{}", self.base_url, path);

        let (body, content_type) = if self.use_binary {
            (
                codec::encode_unary_message_binary(request),
                CONTENT_TYPE_PROTO,
            )
        } else {
            (codec::encode_unary_message_json(request)?, CONTENT_TYPE_JSON)
        };

        let response = self
            .client
            .post(&url)
            .header(CONTENT_TYPE, content_type)
            .header("Connect-Protocol-Version", "1")
            .body(body)
            .send()
            .await
            .map_err(|e| {
                RpcError::new(
                    RpcErrorCode::Unavailable,
                    format!("HTTP request failed: {e}"),
                )
            })?;

        let status = response.status();
        let resp_content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();
        let resp_binary = resp_content_type.starts_with(CONTENT_TYPE_PROTO);

        let bytes = response.bytes().await.map_err(|e| {
            RpcError::new(
                RpcErrorCode::Internal,
                format!("Failed to read response body: {e}"),
            )
        })?;

        // Non-OK status → error response (always JSON per spec)
        if !status.is_success() {
            if let Ok(rpc_error) = codec::decode_error(&bytes) {
                return Err(rpc_error);
            }
            return Err(RpcError::new(
                RpcErrorCode::Unknown,
                format!("HTTP {}: {}", status, String::from_utf8_lossy(&bytes)),
            ));
        }

        // Successful response
        codec::decode_message(&bytes, resp_binary)
    }

    /// Send a server-streaming RPC call.
    ///
    /// Returns a stream that yields each response message. The stream
    /// terminates when the server sends the end-of-stream trailer.
    ///
    /// This is used by generated client stubs — you normally don't call it directly.
    pub async fn server_stream<Req, Res>(
        &self,
        path: &str,
        request: Req,
    ) -> RpcResult<ConnectResponseStream<Res>>
    where
        Req: Message + Serialize,
        Res: Message + DeserializeOwned + Default + 'static,
    {
        let url = format!("{}{}", self.base_url, path);

        // Streaming requests are envelope-wrapped
        let body = codec::encode_envelope(request, self.use_binary)?;
        let content_type = if self.use_binary {
            CONTENT_TYPE_CONNECT_PROTO
        } else {
            CONTENT_TYPE_CONNECT_JSON
        };

        let response = self
            .client
            .post(&url)
            .header(CONTENT_TYPE, content_type)
            .header("Connect-Protocol-Version", "1")
            .body(body)
            .send()
            .await
            .map_err(|e| {
                RpcError::new(
                    RpcErrorCode::Unavailable,
                    format!("HTTP request failed: {e}"),
                )
            })?;

        let status = response.status();

        // For streaming, the server ALWAYS returns 200 per spec.
        // Non-200 means something went very wrong (e.g. routing error).
        if !status.is_success() {
            let bytes = response.bytes().await.map_err(|e| {
                RpcError::new(
                    RpcErrorCode::Internal,
                    format!("Failed to read error response body: {e}"),
                )
            })?;

            if let Ok(rpc_error) = codec::decode_error(&bytes) {
                return Err(rpc_error);
            }
            return Err(RpcError::new(
                RpcErrorCode::Unknown,
                format!("HTTP {}: {}", status, String::from_utf8_lossy(&bytes)),
            ));
        }

        // Determine encoding from response content-type
        let resp_content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();
        let resp_binary = resp_content_type.starts_with(CONTENT_TYPE_CONNECT_PROTO);

        Ok(ConnectResponseStream::new(response, resp_binary))
    }
}

impl ConnectClientBuilder {
    /// Use a custom `reqwest::Client` (for TLS configuration, timeouts, etc.).
    pub fn client(mut self, client: reqwest::Client) -> Self {
        self.client = Some(client);
        self
    }

    /// Use binary protobuf encoding instead of JSON.
    pub fn use_binary(mut self, binary: bool) -> Self {
        self.use_binary = binary;
        self
    }

    /// Build the `ConnectClient`.
    pub fn build(self) -> ConnectClient {
        ConnectClient {
            base_url: self.base_url,
            client: self.client.unwrap_or_else(reqwest::Client::new),
            use_binary: self.use_binary,
        }
    }
}
