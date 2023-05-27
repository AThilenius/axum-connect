use std::net::SocketAddr;

use axum::{routing::get, Router};
use axum_connect::prelude::*;
use tower_http::cors::CorsLayer;

use proto::grpc::testing::*;

mod proto {
    pub mod grpc {
        pub mod testing {
            include!(concat!(env!("OUT_DIR"), "/grpc.testing.rs"));
        }
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/status", get(status))
        .rpc(TestService::empty_call(empty_call))
        .rpc(TestService::unary_call(unary_call))
        .rpc(TestService::fail_unary_call(fail_unary_call));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3030));
    println!("listening on http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.layer(CorsLayer::very_permissive()).into_make_service())
        .await
        .unwrap();
}

async fn status() -> &'static str {
    "OK\n"
}

async fn empty_call(_: Empty) -> Empty {
    Empty {}
}

async fn unary_call(request: SimpleRequest) -> Result<SimpleResponse, RpcError> {
    // if leadingMetadata := request.Header().Values(leadingMetadataKey); len(leadingMetadata) != 0 {
    // 	for _, value := range leadingMetadata {
    // 		response.Header().Add(leadingMetadataKey, value)
    // 	}
    // }
    // if trailingMetadata := request.Header().Values(trailingMetadataKey); len(trailingMetadata) != 0 {
    // 	for _, value := range trailingMetadata {
    // 		decodedTrailingMetadata, err := connect.DecodeBinaryHeader(value)
    // 		if err != nil {
    // 			return nil, err
    // 		}
    // 		response.Trailer().Add(trailingMetadataKey, connect.EncodeBinaryHeader(decodedTrailingMetadata))
    // 	}
    // }
    // response.Header().Set("Request-Protocol", request.Peer().Protocol)
    // return response, nil

    if let Some(response_status) = &request.response_status {
        if response_status.code != 0 {
            return Err(RpcError {
                code: rpc_error_code_from_i32(response_status.code)
                    .unwrap()
                    .to_owned(),
                message: response_status.message.clone(),
                details: vec![],
            });
        }
    }

    let payload = new_server_payload(request.response_type(), request.response_size)?;
    let response = SimpleResponse {
        payload: Some(payload),
        ..Default::default()
    };

    Ok(response)
}

async fn fail_unary_call(_: SimpleRequest) -> RpcError {
    RpcError {
        code: RpcErrorCode::ResourceExhausted,
        message: "soirée 🎉".to_owned(),
        details: vec![
            // ("reason", ErrorDetail {

            // }).into(),
            // ("domain", "connect-crosstest").into(),
        ],
    }
}

fn new_server_payload(payload_type: PayloadType, size: i32) -> Result<Payload, RpcError> {
    if size < 0 {
        return Err(RpcError::new(
            RpcErrorCode::Internal,
            format!("requested a response with invalid length {}", size),
        ));
    }
    let body = vec![0; size as usize];
    match payload_type {
        PayloadType::Compressable => Ok(Payload {
            r#type: PayloadType::Compressable as i32,
            body,
        }),
    }
}

pub fn rpc_error_code_from_i32(num: i32) -> Option<RpcErrorCode> {
    match num {
        1 => Some(RpcErrorCode::Canceled),
        2 => Some(RpcErrorCode::Unknown),
        3 => Some(RpcErrorCode::InvalidArgument),
        4 => Some(RpcErrorCode::DeadlineExceeded),
        5 => Some(RpcErrorCode::NotFound),
        6 => Some(RpcErrorCode::AlreadyExists),
        7 => Some(RpcErrorCode::PermissionDenied),
        8 => Some(RpcErrorCode::ResourceExhausted),
        9 => Some(RpcErrorCode::FailedPrecondition),
        10 => Some(RpcErrorCode::Aborted),
        11 => Some(RpcErrorCode::OutOfRange),
        12 => Some(RpcErrorCode::Unimplemented),
        13 => Some(RpcErrorCode::Internal),
        14 => Some(RpcErrorCode::Unavailable),
        15 => Some(RpcErrorCode::DataLoss),
        16 => Some(RpcErrorCode::Unauthenticated),
        _ => None,
    }
}

pub fn i32_to_rpc_error_code(rpc_error_code: RpcErrorCode) -> i32 {
    match rpc_error_code {
        RpcErrorCode::Canceled => 1,
        RpcErrorCode::Unknown => 2,
        RpcErrorCode::InvalidArgument => 3,
        RpcErrorCode::DeadlineExceeded => 4,
        RpcErrorCode::NotFound => 5,
        RpcErrorCode::AlreadyExists => 6,
        RpcErrorCode::PermissionDenied => 7,
        RpcErrorCode::ResourceExhausted => 8,
        RpcErrorCode::FailedPrecondition => 9,
        RpcErrorCode::Aborted => 10,
        RpcErrorCode::OutOfRange => 11,
        RpcErrorCode::Unimplemented => 12,
        RpcErrorCode::Internal => 13,
        RpcErrorCode::Unavailable => 14,
        RpcErrorCode::DataLoss => 15,
        RpcErrorCode::Unauthenticated => 16,
    }
}
