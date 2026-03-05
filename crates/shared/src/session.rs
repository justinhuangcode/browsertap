use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::protocol::{ConsoleEvent, NetworkEvent, SocketState};

/// Maximum number of console events buffered per session.
pub const MAX_CONSOLE_BUFFER: usize = 500;

/// Maximum number of network events buffered per session.
pub const MAX_NETWORK_BUFFER: usize = 200;

/// Heartbeat interval expected from browser runtime.
pub const HEARTBEAT_INTERVAL_SECS: u64 = 5;

/// Session expires if no heartbeat received within this duration.
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 45;

/// A live browser session connected to the daemon.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub codename: String,
    pub url: String,
    pub title: String,
    pub user_agent: String,
    pub top_origin: String,
    pub socket_state: SocketState,
    pub connected_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
    pub console_buffer: Vec<ConsoleEvent>,
    pub network_buffer: Vec<NetworkEvent>,
}

impl Session {
    pub fn new(
        id: Uuid,
        codename: String,
        url: String,
        title: String,
        user_agent: String,
        top_origin: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            codename,
            url,
            title,
            user_agent,
            top_origin,
            socket_state: SocketState::Open,
            connected_at: now,
            last_heartbeat: now,
            console_buffer: Vec::new(),
            network_buffer: Vec::new(),
        }
    }

    /// Check if the session has timed out based on heartbeat.
    pub fn is_stale(&self) -> bool {
        let elapsed = Utc::now()
            .signed_duration_since(self.last_heartbeat)
            .num_seconds();
        elapsed > HEARTBEAT_TIMEOUT_SECS as i64
    }

    /// Update the last heartbeat timestamp.
    pub fn touch(&mut self) {
        self.last_heartbeat = Utc::now();
    }

    /// Append console events, enforcing buffer limit.
    pub fn push_console_events(&mut self, events: Vec<ConsoleEvent>) {
        self.console_buffer.extend(events);
        if self.console_buffer.len() > MAX_CONSOLE_BUFFER {
            let drain = self.console_buffer.len() - MAX_CONSOLE_BUFFER;
            self.console_buffer.drain(..drain);
        }
    }

    /// Append network events, enforcing buffer limit.
    pub fn push_network_events(&mut self, events: Vec<NetworkEvent>) {
        self.network_buffer.extend(events);
        if self.network_buffer.len() > MAX_NETWORK_BUFFER {
            let drain = self.network_buffer.len() - MAX_NETWORK_BUFFER;
            self.network_buffer.drain(..drain);
        }
    }
}

/// Daemon configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// HTTPS listen address (default: 127.0.0.1)
    #[serde(default = "default_host")]
    pub host: String,
    /// HTTPS listen port (default: 4455)
    #[serde(default = "default_port")]
    pub port: u16,
    /// Path to TLS certificate file
    #[serde(default)]
    pub cert_path: Option<String>,
    /// Path to TLS private key file
    #[serde(default)]
    pub key_path: Option<String>,
}

fn default_host() -> String {
    "127.0.0.1".into()
}

fn default_port() -> u16 {
    4455
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            cert_path: None,
            key_path: None,
        }
    }
}

/// Project-level configuration (browsertap.toml).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    #[serde(default)]
    pub app_label: Option<String>,
    #[serde(default)]
    pub app_url: Option<String>,
    #[serde(default)]
    pub daemon_url: Option<String>,
    #[serde(default)]
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub smoke: SmokeConfig,
}

/// Smoke test configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SmokeConfig {
    /// Default routes to test.
    #[serde(default)]
    pub defaults: Vec<String>,
    /// Named presets of route lists.
    #[serde(default)]
    pub presets: std::collections::HashMap<String, Vec<String>>,
    /// Known redirects (from -> to).
    #[serde(default)]
    pub redirects: std::collections::HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_heartbeat_and_stale() {
        let mut session = Session::new(
            Uuid::new_v4(),
            "test-fox".into(),
            "http://localhost:3000".into(),
            "Test".into(),
            "Mozilla/5.0".into(),
            "http://localhost:3000".into(),
        );

        assert!(!session.is_stale());
        session.touch();
        assert!(!session.is_stale());
    }

    #[test]
    fn console_buffer_limit() {
        let mut session = Session::new(
            Uuid::new_v4(),
            "test-owl".into(),
            "http://localhost:3000".into(),
            "Test".into(),
            "Mozilla/5.0".into(),
            "http://localhost:3000".into(),
        );

        let events: Vec<ConsoleEvent> = (0..600)
            .map(|i| ConsoleEvent {
                id: format!("evt-{i}"),
                timestamp: i as i64,
                level: crate::protocol::ConsoleLevel::Log,
                args: vec![serde_json::json!(format!("message {i}"))],
            })
            .collect();

        session.push_console_events(events);
        assert_eq!(session.console_buffer.len(), MAX_CONSOLE_BUFFER);
    }
}
