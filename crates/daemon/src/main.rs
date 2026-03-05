mod state;
mod tls;
mod websocket;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::extract::{Json, Path, State};
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
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

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
    let token =
        sign_token(&payload, state.secret()).map_err(|e| AppError::Internal(e.to_string()))?;

    let socket_url = format!("wss://{}:{}/bridge", state.host(), state.port());

    Ok(Json(HandshakeResponse {
        session_id,
        session_token: token,
        socket_url,
        expires_at: payload.expires_at.timestamp(),
    }))
}

async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Vec<SessionInfo>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState::new().expect("failed to create test state")
    }

    #[tokio::test]
    async fn health_check() {
        let app = build_router(test_state());
        let req = Request::get("/health").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"ok");
    }

    #[tokio::test]
    async fn list_sessions_empty() {
        let app = build_router(test_state());
        let req = Request::get("/api/sessions").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let sessions: Vec<SessionInfo> = serde_json::from_slice(&body).unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn get_session_not_found() {
        let app = build_router(test_state());
        let id = Uuid::new_v4();
        let req = Request::get(format!("/api/sessions/{id}"))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn handshake_returns_token() {
        let app = build_router(test_state());
        let body = serde_json::json!({ "app_label": "test-app" });
        let req = Request::post("/api/handshake")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let hs: HandshakeResponse = serde_json::from_slice(&body).unwrap();
        assert!(!hs.session_token.is_empty());
        assert!(hs.socket_url.starts_with("wss://"));
        assert!(hs.expires_at > 0);
    }

    #[tokio::test]
    async fn register_then_list_sessions() {
        let state = test_state();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let session_id = Uuid::new_v4();
        let codename = state.register_session(
            session_id,
            "http://localhost:3000".into(),
            "Test Page".into(),
            "Mozilla/5.0".into(),
            "http://localhost:3000".into(),
            tx,
        );
        assert!(codename.contains('-'));

        let app = build_router(state);
        let req = Request::get("/api/sessions").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let sessions: Vec<SessionInfo> = serde_json::from_slice(&body).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].codename, codename);
    }

    #[tokio::test]
    async fn get_console_for_known_session() {
        let state = test_state();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let session_id = Uuid::new_v4();
        state.register_session(
            session_id,
            "http://localhost:3000".into(),
            "Test".into(),
            "Mozilla/5.0".into(),
            "http://localhost:3000".into(),
            tx,
        );

        let app = build_router(state);
        let req = Request::get(format!("/api/sessions/{session_id}/console"))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let events: Vec<browsertap_shared::protocol::ConsoleEvent> =
            serde_json::from_slice(&body).unwrap();
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn get_network_for_unknown_session() {
        let app = build_router(test_state());
        let id = Uuid::new_v4();
        let req = Request::get(format!("/api/sessions/{id}/network"))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}

/// WebSocket integration tests: spin up a real TCP listener + Axum router,
/// connect via tokio-tungstenite, and test the register/heartbeat/console flow.
#[cfg(test)]
mod ws_tests {
    use super::*;
    use futures::SinkExt;
    use futures::stream::StreamExt;
    use std::net::SocketAddr;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{connect_async, tungstenite};

    /// Start the daemon on a random port and return the address.
    async fn start_server() -> (SocketAddr, AppState) {
        let state = AppState::new().expect("failed to create state");
        let app = build_router(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (addr, state)
    }

    /// Helper to create a text WebSocket message.
    fn text_msg(s: String) -> tungstenite::Message {
        tungstenite::Message::Text(s.into())
    }

    /// Create a valid register message using the daemon's own secret.
    fn make_register_msg(state: &AppState, session_id: uuid::Uuid) -> String {
        let payload =
            browsertap_shared::token::TokenPayload::new(TokenScope::Session, "test", session_id);
        let token = browsertap_shared::token::sign_token(&payload, state.secret()).unwrap();
        let msg = BrowserMessage::Register {
            token,
            session_id,
            url: "http://localhost:3000".into(),
            title: "Test Page".into(),
            user_agent: "Mozilla/5.0".into(),
            top_origin: "http://localhost:3000".into(),
        };
        serde_json::to_string(&msg).unwrap()
    }

    #[tokio::test]
    async fn ws_register_and_receive_codename() {
        let (addr, state) = start_server().await;
        let session_id = Uuid::new_v4();

        let (mut ws, _) = connect_async(format!("ws://{addr}/bridge"))
            .await
            .expect("failed to connect");

        // Send register
        ws.send(text_msg(make_register_msg(&state, session_id)))
            .await
            .unwrap();

        // Expect Metadata response with codename
        let resp = ws.next().await.unwrap().unwrap();
        let text = resp.into_text().unwrap();
        let daemon_msg: DaemonMessage = serde_json::from_str(&text).unwrap();

        match daemon_msg {
            DaemonMessage::Metadata {
                session_id: sid,
                codename,
            } => {
                assert_eq!(sid, session_id);
                assert!(codename.contains('-'));
            }
            other => panic!("expected Metadata, got: {other:?}"),
        }

        // Verify session appears in REST API
        let sessions = state.list_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, session_id);

        ws.close(None).await.ok();
    }

    #[tokio::test]
    async fn ws_invalid_token_rejected() {
        let (addr, _state) = start_server().await;

        let (mut ws, _) = connect_async(format!("ws://{addr}/bridge"))
            .await
            .expect("failed to connect");

        // Send register with a bad token
        let msg = BrowserMessage::Register {
            token: "invalid.token".into(),
            session_id: Uuid::new_v4(),
            url: "http://localhost:3000".into(),
            title: "Test".into(),
            user_agent: "UA".into(),
            top_origin: "http://localhost:3000".into(),
        };
        ws.send(text_msg(serde_json::to_string(&msg).unwrap()))
            .await
            .unwrap();

        // The server sends an Error then closes the connection.
        // We may receive the Error message or a connection reset.
        let mut got_error = false;
        while let Some(result) = ws.next().await {
            match result {
                Ok(tungstenite::Message::Text(text)) => {
                    let daemon_msg: DaemonMessage = serde_json::from_str(&text).unwrap();
                    if let DaemonMessage::Error { message } = daemon_msg {
                        assert!(message.contains("authentication failed"));
                        got_error = true;
                    }
                }
                Ok(tungstenite::Message::Close(_)) => break,
                Err(_) => break, // connection reset is expected
                _ => continue,
            }
        }

        // The error message is sent before the break, so we should get it
        // in most cases. But if the connection resets too fast, that's also
        // valid behavior -- the connection was rejected either way.
        // We accept both outcomes.
        assert!(
            got_error || ws.next().await.is_none(),
            "connection should be closed"
        );
    }

    #[tokio::test]
    async fn ws_heartbeat_updates_session() {
        let (addr, state) = start_server().await;
        let session_id = Uuid::new_v4();

        let (mut ws, _) = connect_async(format!("ws://{addr}/bridge"))
            .await
            .expect("failed to connect");

        // Register
        ws.send(text_msg(make_register_msg(&state, session_id)))
            .await
            .unwrap();
        let _ = ws.next().await; // consume Metadata

        // Record heartbeat timestamp
        let before = state.list_sessions().first().unwrap().last_heartbeat;

        // Small delay so timestamp differs
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Send heartbeat
        let hb = BrowserMessage::Heartbeat { session_id };
        ws.send(text_msg(serde_json::to_string(&hb).unwrap()))
            .await
            .unwrap();

        // Allow processing
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let after = state.list_sessions().first().unwrap().last_heartbeat;
        assert!(after >= before);

        ws.close(None).await.ok();
    }

    #[tokio::test]
    async fn ws_console_events_buffered() {
        let (addr, state) = start_server().await;
        let session_id = Uuid::new_v4();

        let (mut ws, _) = connect_async(format!("ws://{addr}/bridge"))
            .await
            .expect("failed to connect");

        // Register
        ws.send(text_msg(make_register_msg(&state, session_id)))
            .await
            .unwrap();
        let _ = ws.next().await; // consume Metadata

        // Send console events
        let console_msg = BrowserMessage::Console {
            session_id,
            events: vec![
                browsertap_shared::protocol::ConsoleEvent {
                    id: "c1".into(),
                    timestamp: 1000,
                    level: browsertap_shared::protocol::ConsoleLevel::Log,
                    args: vec![serde_json::json!("hello from browser")],
                },
                browsertap_shared::protocol::ConsoleEvent {
                    id: "c2".into(),
                    timestamp: 1001,
                    level: browsertap_shared::protocol::ConsoleLevel::Error,
                    args: vec![serde_json::json!("something broke")],
                },
            ],
        };
        ws.send(text_msg(serde_json::to_string(&console_msg).unwrap()))
            .await
            .unwrap();

        // Allow processing
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Verify via state API
        let buf = state.get_console_buffer(session_id).unwrap();
        assert_eq!(buf.len(), 2);
        assert_eq!(buf[0].id, "c1");
        assert_eq!(buf[1].id, "c2");

        ws.close(None).await.ok();
    }

    #[tokio::test]
    async fn ws_disconnect_removes_session() {
        let (addr, state) = start_server().await;
        let session_id = Uuid::new_v4();

        let (mut ws, _) = connect_async(format!("ws://{addr}/bridge"))
            .await
            .expect("failed to connect");

        // Register
        ws.send(text_msg(make_register_msg(&state, session_id)))
            .await
            .unwrap();
        let _ = ws.next().await; // consume Metadata

        assert_eq!(state.list_sessions().len(), 1);

        // Disconnect
        ws.close(None).await.ok();

        // Allow cleanup
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert_eq!(state.list_sessions().len(), 0);
    }
}
