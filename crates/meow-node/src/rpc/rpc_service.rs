//! Axum-based HTTP RPC service that exposes node functionality to clients.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use meow_nakamoto::{mempool::error::MempoolError, miner::error::MinerError};
use meow_types::{
    address::Address,
    digest::Digest,
    transaction::{SignedTransaction, Transaction},
};

use serde::Serialize;
use tokio::{net::TcpListener, sync::watch};

use crate::rpc::{
    error::RpcServiceError,
    rpc_handler::{RpcHandler, error::RpcHandlerError},
};

/// The result type related to the RPC service.
pub type Result<T> = std::result::Result<T, RpcServiceError>;

/// Maximum number of addresses accepted by a single `GET /objects` request.
pub const MAX_GET_OBJECTS_ADDRESSES: usize = 100;

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
/// - `POST /simulate-transaction` to simulate an unsigned transaction without committing it.
/// - `GET /object/{addr}` to fetch the latest live object by address.
/// - `GET /objects?address=...&address=...` to fetch live objects by a list of addresses.
/// - `GET /objects_owned/{owner}` to fetch all the live objects owned by an address.
/// - `GET /transaction/{digest}` to fetch a committed transaction by digest.
/// - `GET /transaction-result/{digest}` to fetch an execution result by transaction digest.
/// - `GET /block/{digest}` to fetch a block by its hash.
/// - `GET /block-snapshot/{digest}` to fetch the state snapshot at a given block.
/// - `GET /chain-head` to fetch the current chain head digest.
/// - `GET /blocks-since/{height}` to fetch blocks from a given height onwards.
/// - `GET /state-snapshot` to fetch a full state snapshot.
pub fn router(state: RpcState) -> Router {
    Router::new()
        .route("/submit-transaction", post(submit_transaction))
        .route("/simulate-transaction", post(simulate_transaction))
        .route("/object/{addr}", get(get_object))
        .route("/objects", get(get_objects))
        .route("/objects_owned/{owner}", get(get_objects_owned))
        .route("/transaction/{digest}", get(get_transaction))
        .route("/transaction-result/{digest}", get(get_transaction_result))
        .route("/block/{digest}", get(get_block))
        .route("/block-snapshot/{digest}", get(get_block_snapshot))
        .route("/chain-head", get(get_chain_head))
        .route("/blocks-since/{height}", get(get_blocks_since))
        .route("/state-snapshot", get(get_state_snapshot))
        .with_state(state)
}

/// POST /submit-transaction — submit a `SignedTransaction` (JSON-encoded).
///
/// Returns `202 Accepted` on success. Validation and mempool errors are mapped to
/// structured `4xx` responses.
///
/// This handler only submits locally; network propagation is handled outside the RPC layer.
async fn submit_transaction(
    State(state): State<RpcState>,
    Json(signed_transaction): Json<SignedTransaction>,
) -> impl IntoResponse {
    match state.handler.submit_transaction(signed_transaction).await {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(RpcHandlerError::MinerError(err)) => match err {
            MinerError::MempoolError(err) => mempool_error_response(err),
            MinerError::SimulationError(_) => unreachable!("submit_transaction never simulates"),
            MinerError::ChainError(_) => {
                unreachable!("submit_transaction never interacts with the chain")
            }
        },
    }
}

/// POST /simulate-transaction — simulate a `Transaction` (JSON-encoded, unsigned) without committing it.
///
/// Returns `200 OK` on success. Validation, mempool, and simulation errors are mapped to
/// structured `4xx` responses.
///
/// This handler only simulates locally; network propagation is handled outside the RPC layer.
async fn simulate_transaction(
    State(state): State<RpcState>,
    Json(transaction): Json<Transaction>,
) -> impl IntoResponse {
    match state.handler.simulate_transaction(transaction).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(RpcHandlerError::MinerError(err)) => match err {
            MinerError::MempoolError(err) => mempool_error_response(err),
            MinerError::SimulationError(err) => {
                error_response(StatusCode::BAD_REQUEST, "simulation_error", err.to_string())
            }
            MinerError::ChainError(_) => {
                unreachable!("simulate_transaction never interacts with the chain")
            }
        },
    }
}

/// GET /object/{addr} — returns the live `Object` at the given address, or `null` if not found.
///
/// Returns `400` for invalid address format.
async fn get_object(
    State(state): State<RpcState>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    let address = match parse_address(&address) {
        Ok(address) => address,
        Err(response) => return response,
    };
    Json(state.handler.get_object(&address).await).into_response()
}

/// GET /objects?address=...&address=... — returns live `Object`s for the given addresses.
///
/// Each `address` query parameter must be a 0x-prefixed hex string. Returns `400` if any
/// address is invalid or the number of addresses exceeds `MAX_GET_OBJECTS_ADDRESSES`.
/// Each entry in the response is `null` if the object was not found.
async fn get_objects(
    State(state): State<RpcState>,
    Query(params): Query<Vec<(String, String)>>,
) -> impl IntoResponse {
    let raw_addresses: Vec<&str> = params
        .iter()
        .filter(|(k, _)| k == "address")
        .map(|(_, v)| v.as_str())
        .collect();

    if raw_addresses.len() > MAX_GET_OBJECTS_ADDRESSES {
        return error_response(
            StatusCode::BAD_REQUEST,
            "too_many_addresses",
            format!("too many addresses: limit is {MAX_GET_OBJECTS_ADDRESSES} per request"),
        );
    }
    let mut addresses = Vec::new();
    for raw in &raw_addresses {
        let addr = match parse_address(raw) {
            Ok(address) => address,
            Err(response) => return response,
        };
        addresses.push(addr);
    }
    Json(state.handler.get_objects(&addresses).await).into_response()
}

/// GET /objects_owned/{owner} — returns all the live `Object`s owned by the given address.
///
/// Returns `400` for invalid address format.
async fn get_objects_owned(
    State(state): State<RpcState>,
    Path(owner): Path<String>,
) -> impl IntoResponse {
    let owner = match parse_address(&owner) {
        Ok(address) => address,
        Err(response) => return response,
    };
    Json(state.handler.get_objects_owned(&owner).await).into_response()
}

/// GET /transaction/{digest} — returns the committed `SignedTransaction` by digest, or `null` if not found.
///
/// Returns `400` for invalid digest format.
async fn get_transaction(
    State(state): State<RpcState>,
    Path(digest): Path<String>,
) -> impl IntoResponse {
    let digest = match parse_digest(&digest) {
        Ok(digest) => digest,
        Err(response) => return response,
    };
    Json(state.handler.get_transaction(&digest).await).into_response()
}

/// GET /transaction-result/{digest} — returns the `ExecutionResult` for a committed transaction, or `null` if not found.
///
/// Returns `400` for invalid digest format.
async fn get_transaction_result(
    State(state): State<RpcState>,
    Path(digest): Path<String>,
) -> impl IntoResponse {
    let digest = match parse_digest(&digest) {
        Ok(digest) => digest,
        Err(response) => return response,
    };
    Json(state.handler.get_transaction_result(&digest).await).into_response()
}

/// GET /block/{digest} — returns the `Block` by hash, or `null` if not found.
///
/// Returns `400` for an invalid digest.
async fn get_block(State(state): State<RpcState>, Path(digest): Path<String>) -> impl IntoResponse {
    let digest = match parse_digest(&digest) {
        Ok(digest) => digest,
        Err(response) => return response,
    };
    Json(state.handler.get_block(&digest).await).into_response()
}

/// GET /block-snapshot/{digest} — returns the full state snapshot at the given block hash, or
/// `null` if not found.
///
/// Returns `400` for an invalid digest.
async fn get_block_snapshot(
    State(state): State<RpcState>,
    Path(digest): Path<String>,
) -> impl IntoResponse {
    let digest = match parse_digest(&digest) {
        Ok(digest) => digest,
        Err(response) => return response,
    };
    Json(state.handler.get_block_snapshot(&digest).await).into_response()
}

/// GET /chain-head — returns the current chain head digest (base58 string).
async fn get_chain_head(State(state): State<RpcState>) -> impl IntoResponse {
    Json(state.handler.get_chain_head().await).into_response()
}

/// GET /blocks-since/{height} — returns all blocks from the given height onwards.
/// Used by nodes to synchronize the chain after joining the network.
async fn get_blocks_since(
    State(state): State<RpcState>,
    Path(height): Path<u64>,
) -> impl IntoResponse {
    Json(state.handler.get_blocks_since(height).await).into_response()
}

/// GET /state-snapshot — returns a full state snapshot at the current chain head.
/// Used by nodes when full chain synchronization is required.
async fn get_state_snapshot(State(state): State<RpcState>) -> impl IntoResponse {
    Json(state.handler.get_state_snapshot().await).into_response()
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

/// Parses a raw string into an `Address`, returning a `400` error response on failure.
#[allow(clippy::result_large_err)]
fn parse_address(raw: &str) -> std::result::Result<Address, axum::response::Response> {
    raw.parse().map_err(|err| {
        error_response(
            StatusCode::BAD_REQUEST,
            "invalid_address",
            format!("invalid address '{raw}': {err}"),
        )
    })
}

/// Parses a raw string into a `Digest`, returning a `400` error response on failure.
#[allow(clippy::result_large_err)]
fn parse_digest(raw: &str) -> std::result::Result<Digest, axum::response::Response> {
    raw.parse().map_err(|err| {
        error_response(
            StatusCode::BAD_REQUEST,
            "invalid_digest",
            format!("invalid digest '{raw}': {err}"),
        )
    })
}

/// Maps a `MempoolError` to a structured HTTP error response.
fn mempool_error_response(err: MempoolError) -> axum::response::Response {
    let (status, code) = match &err {
        MempoolError::TransactionValidationError(_) => {
            (StatusCode::BAD_REQUEST, "invalid_transaction")
        }
        MempoolError::DuplicateTransaction { .. } => {
            (StatusCode::CONFLICT, "duplicate_transaction")
        }
        MempoolError::ObjectNotFound { .. }
        | MempoolError::InvalidObjectVersion { .. }
        | MempoolError::InvalidObjectDigest { .. } => {
            (StatusCode::BAD_REQUEST, "invalid_object_reference")
        }
        MempoolError::MempoolFull { .. } => (StatusCode::SERVICE_UNAVAILABLE, "mempool_full"),
    };
    error_response(status, code, err.to_string())
}
