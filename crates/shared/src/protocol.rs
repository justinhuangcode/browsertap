use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Browser → Daemon messages ───────────────────────────────────────────────

/// Messages sent from browser runtime to daemon via WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BrowserMessage {
    /// Browser registers a new session with the daemon.
    Register {
        token: String,
        session_id: Uuid,
        url: String,
        title: String,
        user_agent: String,
        top_origin: String,
    },
    /// Periodic heartbeat to keep the session alive.
    Heartbeat {
        session_id: Uuid,
    },
    /// Result of a command executed in the browser.
    CommandResult {
        session_id: Uuid,
        command_id: String,
        result: CommandResult,
    },
    /// Batch of console events captured from the page.
    Console {
        session_id: Uuid,
        events: Vec<ConsoleEvent>,
    },
    /// Network events captured from the page.
    Network {
        session_id: Uuid,
        events: Vec<NetworkEvent>,
    },
}

// ─── Daemon → Browser messages ───────────────────────────────────────────────

/// Messages sent from daemon to browser runtime via WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DaemonMessage {
    /// Session metadata after successful registration.
    Metadata {
        session_id: Uuid,
        codename: String,
    },
    /// Command to execute in the browser context.
    Command {
        session_id: Uuid,
        command: BrowserCommand,
    },
    /// Disconnect notification.
    Disconnect {
        reason: String,
    },
    /// Error response.
    Error {
        message: String,
    },
}

// ─── Commands (CLI → Daemon → Browser) ───────────────────────────────────────

/// Commands that can be sent to a browser session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BrowserCommand {
    /// Execute JavaScript in the page context.
    RunScript {
        id: String,
        code: String,
        #[serde(default)]
        capture_console: bool,
    },
    /// Take a screenshot of the page or a specific element.
    Screenshot {
        id: String,
        #[serde(default)]
        selector: Option<String>,
        #[serde(default = "default_quality")]
        quality: f32,
        #[serde(default)]
        hooks: Vec<ScreenshotHook>,
    },
    /// Click an element by CSS selector.
    Click {
        id: String,
        selector: String,
    },
    /// Navigate to a URL.
    Navigate {
        id: String,
        url: String,
    },
    /// Discover interactive selectors on the page.
    DiscoverSelectors {
        id: String,
    },
}

fn default_quality() -> f32 {
    0.85
}

impl BrowserCommand {
    pub fn id(&self) -> &str {
        match self {
            BrowserCommand::RunScript { id, .. }
            | BrowserCommand::Screenshot { id, .. }
            | BrowserCommand::Click { id, .. }
            | BrowserCommand::Navigate { id, .. }
            | BrowserCommand::DiscoverSelectors { id, .. } => id,
        }
    }
}

/// Pre-screenshot hooks to prepare the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ScreenshotHook {
    ScrollIntoView { selector: String },
    WaitForSelector { selector: String, timeout_ms: u64 },
    WaitForIdle { timeout_ms: u64 },
    Wait { ms: u64 },
    Script { code: String },
}

// ─── Command results ─────────────────────────────────────────────────────────

/// Result of a command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Screenshot-specific result data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotData {
    pub mime_type: String,
    pub base64: String,
    pub width: u32,
    pub height: u32,
    pub renderer: String,
}

// ─── Telemetry events ────────────────────────────────────────────────────────

/// Console event captured from the browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleEvent {
    pub id: String,
    pub timestamp: i64,
    pub level: ConsoleLevel,
    pub args: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleLevel {
    Log,
    Info,
    Warn,
    Error,
    Debug,
}

/// Network event captured from the browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEvent {
    pub id: String,
    pub timestamp: i64,
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ─── REST API types ──────────────────────────────────────────────────────────

/// POST /api/handshake - request body.
#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeRequest {
    pub app_label: String,
}

/// POST /api/handshake - response body.
#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub session_id: Uuid,
    pub session_token: String,
    pub socket_url: String,
    pub expires_at: i64,
}

/// GET /api/sessions - single session info.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub codename: String,
    pub url: String,
    pub title: String,
    pub user_agent: String,
    pub socket_state: SocketState,
    pub connected_at: i64,
    pub last_heartbeat: i64,
    pub console_buffer_size: usize,
    pub network_buffer_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SocketState {
    Open,
    Closed,
}

/// POST /api/sessions/{id}/command - request body.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandRequest {
    pub command: BrowserCommand,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    30_000
}

/// POST /api/sessions/{id}/command - response body.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResponse {
    pub result: CommandResult,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_message_serialization() {
        let msg = BrowserMessage::Register {
            token: "test-token".into(),
            session_id: Uuid::new_v4(),
            url: "http://localhost:3000".into(),
            title: "Test Page".into(),
            user_agent: "Mozilla/5.0".into(),
            top_origin: "http://localhost:3000".into(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"kind\":\"register\""));

        let decoded: BrowserMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, BrowserMessage::Register { .. }));
    }

    #[test]
    fn command_serialization() {
        let cmd = BrowserCommand::RunScript {
            id: "cmd-1".into(),
            code: "document.title".into(),
            capture_console: true,
        };

        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"type\":\"runScript\""));
    }
}
