mod state;
mod tls;
mod websocket;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::extract::{Path, State, Json};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use uuid::Uuid;

use browsertap_shared::protocol::*;
use browsertap_shared::token::*;

use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "browsertapd=info,tower_http=info".into()),
        )
        .init();

    let state = AppState::new()?;
    let addr = SocketAddr::from(([127, 0, 0, 1], state.port()));

    let app = build_router(state.clone());

    // Try TLS first, fall back to plain HTTP for development
    match tls::create_tls_acceptor(&state) {
        Ok(acceptor) => {
            info!("browsertapd listening on https://{addr}");
            let listener = tokio::net::TcpListener::bind(addr).await?;
            loop {
                let (stream, _peer) = listener.accept().await?;
                let acceptor = acceptor.clone();
                let app = app.clone();
                tokio::spawn(async move {
                    match acceptor.accept(stream).await {
                        Ok(tls_stream) => {
                            let io = hyper_util::rt::TokioIo::new(tls_stream);
                            let service = hyper_util::service::TowerToHyperService::new(app);
                            if let Err(e) = hyper_util::server::conn::auto::Builder::new(
                                hyper_util::rt::TokioExecutor::new(),
                            )
                            .serve_connection(io, service)
                            .await
                            {
                                tracing::debug!("connection error: {e}");
                            }
                        }
                        Err(e) => tracing::debug!("TLS accept error: {e}"),
                    }
                });
            }
        }
        Err(e) => {
            tracing::warn!("TLS not available ({e}), starting plain HTTP (dev mode)");
            info!("browsertapd listening on http://{addr}");
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app).await?;
            Ok(())
        }
    }
}

fn build_router(state: AppState) -> Router {
    Router::new()
        // Health check
        .route("/health", get(health))
        // Session management
        .route("/api/sessions", get(list_sessions))
        .route("/api/sessions/{session_id}", get(get_session))
        .route("/api/sessions/{session_id}/command", post(send_command))
        .route("/api/sessions/{session_id}/console", get(get_console))
        .route("/api/sessions/{session_id}/network", get(get_network))
        // Handshake (mints session tokens for browser runtime)
        .route("/api/handshake", post(handshake))
        // WebSocket bridge
        .route("/bridge", get(websocket::ws_handler))
        // Middleware
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(Arc::new(state))
}

// ─── Route handlers ──────────────────────────────────────────────────────────

async fn health() -> &'static str {
    "ok"
}

async fn handshake(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HandshakeRequest>,
) -> Result<Json<HandshakeResponse>, AppError> {
    let session_id = Uuid::new_v4();
    let payload = TokenPayload::new(TokenScope::Session, &req.app_label, session_id);
    let token = sign_token(&payload, state.secret())
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let socket_url = format!("wss://{}:{}/bridge", state.host(), state.port());

    Ok(Json(HandshakeResponse {
        session_id,
        session_token: token,
        socket_url,
        expires_at: payload.expires_at.timestamp(),
    }))
}

async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<SessionInfo>> {
    let sessions = state.list_sessions();
    Json(sessions)
}

async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionInfo>, AppError> {
    let session = state
        .find_session(&session_id)
        .ok_or(AppError::NotFound("session not found".into()))?;
    Ok(Json(session))
}

async fn send_command(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<CommandRequest>,
) -> Result<Json<CommandResponse>, AppError> {
    let result = state
        .send_command(session_id, req.command, req.timeout_ms)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(CommandResponse { result }))
}

async fn get_console(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Vec<ConsoleEvent>>, AppError> {
    let events = state
        .get_console_buffer(session_id)
        .ok_or(AppError::NotFound("session not found".into()))?;
    Ok(Json(events))
}

async fn get_network(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Vec<NetworkEvent>>, AppError> {
    let events = state
        .get_network_buffer(session_id)
        .ok_or(AppError::NotFound("session not found".into()))?;
    Ok(Json(events))
}

// ─── Error handling ──────────────────────────────────────────────────────────

#[allow(dead_code)]
enum AppError {
    NotFound(String),
    Unauthorized(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
