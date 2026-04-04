use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use meow_nakamoto::{mempool::error::MempoolError, miner::error::MinerError};
use meow_types::{address::Address, digest::Digest, transaction::SignedTransaction};
use serde::Serialize;

use crate::rpc::rpc_handler::{RpcHandler, error::RpcHandlerError};

/// Shared application state for RPC handlers.
///
/// Contains the RPC business-logic handler.
#[derive(Clone)]
pub struct RpcState {
    /// Service handling transaction submission, object lookup, and result lookup.
    handler: RpcHandler,
}

/// JSON error body returned by RPC endpoints on non-success status codes.
#[derive(Serialize)]
struct ErrorBody {
    /// Stable, machine-readable error code.
    code: &'static str,
    /// Human-readable error message.
    message: String,
}

impl RpcState {
    /// Creates a new application state with the given RPC handler.
    pub fn new(handler: RpcHandler) -> Self {
        Self { handler }
    }
}

/// Builds the Axum router for node RPC endpoints.
///
/// Registered routes:
/// - `POST /tx` to submit a signed transaction.
/// - `GET /object/{addr}` to fetch the latest live object by address.
/// - `GET /tx/{digest}` to fetch an execution result by transaction digest.
pub fn router(state: RpcState) -> Router {
    Router::new()
        .route("/tx", post(submit_tx))
        .route("/object/{addr}", get(get_object))
        .route("/tx/{digest}", get(get_tx_result))
        .with_state(state)
}

/// POST /tx — submit a `SignedTransaction` (JSON-encoded).
///
/// Returns `202 Accepted` on success. Validation and mempool errors are mapped to
/// structured `4xx` responses.
///
/// This handler only submits locally; network propagation is handled outside the RPC layer.
async fn submit_tx(
    State(state): State<RpcState>,
    Json(tx): Json<SignedTransaction>,
) -> impl IntoResponse {
    match state.handler.submit_tx(tx).await {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(RpcHandlerError::MinerError(err)) => match err {
            MinerError::MempoolError(err) => match err {
                MempoolError::TransactionValidationError(err) => error_response(
                    StatusCode::BAD_REQUEST,
                    "invalid_transaction",
                    format!("invalid transaction: {err}"),
                ),
                MempoolError::DuplicateTransaction(digest) => error_response(
                    StatusCode::CONFLICT,
                    "duplicate_transaction",
                    format!("duplicate transaction: {digest}"),
                ),
                MempoolError::GasCoinNotFound(addr) => error_response(
                    StatusCode::BAD_REQUEST,
                    "gas_coin_not_found",
                    format!("gas coin not found: {addr}"),
                ),
                MempoolError::ObjectNotFound(addr) => error_response(
                    StatusCode::BAD_REQUEST,
                    "invalid_object_reference",
                    format!("invalid object reference: object not found: {addr}"),
                ),
                MempoolError::InvalidObjectVersion {
                    address,
                    expected,
                    found,
                } => error_response(
                    StatusCode::BAD_REQUEST,
                    "invalid_object_reference",
                    format!(
                        "invalid object reference: object {address} has invalid version: expected {expected}, found {found}"
                    ),
                ),
                MempoolError::InvalidObjectDigest {
                    address,
                    expected,
                    found,
                } => error_response(
                    StatusCode::BAD_REQUEST,
                    "invalid_object_reference",
                    format!(
                        "invalid object reference: object {address} has invalid digest: expected {expected}, found {found}"
                    ),
                ),
            },
        },
    }
}

/// GET /object/:addr — returns the live `Object` at the given address.
///
/// Returns `400` for invalid address format and `404` when not found.
async fn get_object(State(state): State<RpcState>, Path(addr): Path<String>) -> impl IntoResponse {
    let addr: Address = match addr.parse() {
        Ok(a) => a,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_address",
                format!("invalid address: {addr} (expected 0x-prefixed hex address)"),
            );
        }
    };
    match state.handler.get_object(&addr).await {
        Ok(Some(obj)) => Json(obj).into_response(),
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "object_not_found",
            format!("object not found: {addr}"),
        ),
        Err(RpcHandlerError::MinerError(err)) => unexpected_miner_error(err),
    }
}

/// GET /tx/:digest — returns the `ExecutionResult` for a committed transaction.
///
/// Returns `400` for invalid digest format and `404` when not found.
async fn get_tx_result(
    State(state): State<RpcState>,
    Path(digest): Path<String>,
) -> impl IntoResponse {
    let digest: Digest = match digest.parse() {
        Ok(digest) => digest,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_digest",
                format!("invalid digest: {digest} (expected base58 digest)"),
            );
        }
    };
    match state.handler.get_tx_result(&digest).await {
        Ok(Some(result)) => Json(result).into_response(),
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "transaction_not_found",
            format!("transaction not found: {digest}"),
        ),
        Err(RpcHandlerError::MinerError(err)) => unexpected_miner_error(err),
    }
}

/// Creates a uniform JSON error response body for all RPC endpoints.
fn error_response(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> axum::response::Response {
    (
        status,
        Json(ErrorBody {
            code,
            message: message.into(),
        }),
    )
        .into_response()
}

/// Maps unexpected miner errors to a uniform JSON error response.
fn unexpected_miner_error(err: MinerError) -> axum::response::Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        format!("unexpected miner error: {err}"),
    )
}
