use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::SinkExt;
use futures::stream::StreamExt;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

use browsertap_shared::protocol::*;

use crate::state::AppState;

/// Axum handler for WebSocket upgrade at /bridge.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle a single WebSocket connection from a browser runtime.
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_sink, mut ws_stream) = socket.split();

    // Channel for sending daemon messages to this socket
    let (tx, mut rx) = mpsc::unbounded_channel::<DaemonMessage>();

    // Spawn writer task: reads from channel, writes to WebSocket
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let json = match serde_json::to_string(&msg) {
                Ok(j) => j,
                Err(e) => {
                    error!("failed to serialize daemon message: {e}");
                    continue;
                }
            };
            if ws_sink.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    let mut session_id: Option<Uuid> = None;

    // Read loop: process messages from browser
    while let Some(result) = ws_stream.next().await {
        let msg = match result {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(e) => {
                warn!("WebSocket error: {e}");
                break;
            }
        };

        let browser_msg: BrowserMessage = match serde_json::from_str(&msg) {
            Ok(m) => m,
            Err(e) => {
                warn!("invalid browser message: {e}");
                let _ = tx.send(DaemonMessage::Error {
                    message: format!("invalid message: {e}"),
                });
                continue;
            }
        };

        match browser_msg {
            BrowserMessage::Register {
                token,
                session_id: sid,
                url,
                title,
                user_agent,
                top_origin,
            } => {
                // Verify the session token
                match state.verify_session_token(&token) {
                    Ok(payload) => {
                        if payload.session_id != sid {
                            let _ = tx.send(DaemonMessage::Error {
                                message: "session ID mismatch".into(),
                            });
                            break;
                        }

                        let codename = state.register_session(
                            sid,
                            url,
                            title,
                            user_agent,
                            top_origin,
                            tx.clone(),
                        );
                        session_id = Some(sid);

                        let _ = tx.send(DaemonMessage::Metadata {
                            session_id: sid,
                            codename: codename.clone(),
                        });

                        info!(
                            session_id = %sid,
                            codename = %codename,
                            "browser registered"
                        );
                    }
                    Err(e) => {
                        warn!("token verification failed: {e}");
                        let _ = tx.send(DaemonMessage::Error {
                            message: format!("authentication failed: {e}"),
                        });
                        break;
                    }
                }
            }

            BrowserMessage::Heartbeat { session_id: sid } => {
                state.heartbeat(sid);
            }

            BrowserMessage::CommandResult {
                command_id, result, ..
            } => {
                state.resolve_command(&command_id, result);
            }

            BrowserMessage::Console {
                session_id: sid,
                events,
            } => {
                state.push_console_events(sid, events);
            }

            BrowserMessage::Network {
                session_id: sid,
                events,
            } => {
                state.push_network_events(sid, events);
            }
        }
    }

    // Cleanup on disconnect
    if let Some(sid) = session_id {
        state.remove_session(sid);
        info!(session_id = %sid, "browser disconnected");
    }

    writer.abort();
}
