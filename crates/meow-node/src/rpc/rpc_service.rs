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
use tokio::{net::TcpListener, sync::watch};

use crate::rpc::{
    error::RpcServiceError,
    rpc_handler::{RpcHandler, error::RpcHandlerError},
};

/// The result type related to the RPC service.
pub type Result<T> = std::result::Result<T, RpcServiceError>;

/// Shared application state for RPC handlers.
///
/// Contains the RPC business-logic handler.
#[derive(Clone)]
pub struct RpcState {
    /// Service handling transaction submission and transaction/object lookups.
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

/// Runs the RPC server with the given TCP listener and application state.
pub async fn run(
    rpc_router: Router,
    tcp_listener: TcpListener,
    mut rpc_shutdown_rx: watch::Receiver<()>,
) -> Result<()> {
    tracing::info!("starting RPC service");

    let result = axum::serve(tcp_listener, rpc_router)
        .with_graceful_shutdown(async move {
            let _ = rpc_shutdown_rx.changed().await;
            tracing::info!("RPC shutdown signal received");
        })
        .await;

    tracing::info!("RPC service stopped");

    result.map_err(Into::into)
}

/// Builds the Axum router for node RPC endpoints.
///
/// Registered routes:
/// - `POST /submit-transaction` to submit a signed transaction.
/// - `GET /object/{addr}` to fetch the latest live object by address.
/// - `GET /objects/{owner}` to fetch all the live objects owned by an address.
/// - `GET /transaction/{digest}` to fetch a committed transaction by digest.
/// - `GET /transaction-result/{digest}` to fetch an execution result by transaction digest.
/// - `GET /blocks-since/{height}` to fetch blocks from a given height onwards (for sync).
pub fn router(state: RpcState) -> Router {
    Router::new()
        .route("/submit-transaction", post(submit_tx))
        .route("/object/{addr}", get(get_object))
        .route("/objects/{owner}", get(get_objects))
        .route("/transaction/{digest}", get(get_transaction))
        .route("/transaction-result/{digest}", get(get_transaction_result))
        .route("/blocks-since/{height}", get(get_blocks_since))
        .with_state(state)
}

/// POST /submit-transaction — submit a `SignedTransaction` (JSON-encoded).
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

/// GET /objects/:owner — returns all the live `Object`s owned by the given address.
///
/// Returns `400` for invalid address format.
async fn get_objects(
    State(state): State<RpcState>,
    Path(owner): Path<String>,
) -> impl IntoResponse {
    let owner: Address = match owner.parse() {
        Ok(a) => a,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_address",
                format!("invalid address: {owner} (expected 0x-prefixed hex address)"),
            );
        }
    };
    match state.handler.get_objects(&owner).await {
        Ok(objects) => Json(objects).into_response(),
        Err(RpcHandlerError::MinerError(err)) => unexpected_miner_error(err),
    }
}

/// GET /transaction/:digest — returns the committed `SignedTransaction` by digest.
///
/// Returns `400` for invalid digest format and `404` when not found.
async fn get_transaction(
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
    match state.handler.get_transaction(&digest).await {
        Ok(Some(tx)) => Json(tx).into_response(),
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "transaction_not_found",
            format!("transaction not found: {digest}"),
        ),
        Err(RpcHandlerError::MinerError(err)) => unexpected_miner_error(err),
    }
}

/// GET /transaction-result/:digest — returns the `ExecutionResult` for a committed transaction.
///
/// Returns `400` for invalid digest format and `404` when not found.
async fn get_transaction_result(
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
    match state.handler.get_transaction_result(&digest).await {
        Ok(Some(result)) => Json(result).into_response(),
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "transaction_not_found",
            format!("transaction not found: {digest}"),
        ),
        Err(RpcHandlerError::MinerError(err)) => unexpected_miner_error(err),
    }
}

/// GET /blocks-since/:height — returns all blocks from the given height onwards.
/// Used by nodes to synchronize the chain after joining the network.
async fn get_blocks_since(
    State(state): State<RpcState>,
    Path(height): Path<u64>,
) -> impl IntoResponse {
    match state.handler.get_blocks_since(height).await {
        Ok(blocks) => Json(blocks).into_response(),
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
