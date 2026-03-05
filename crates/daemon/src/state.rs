use std::sync::Arc;

use anyhow::{Context, Result};
use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};
use uuid::Uuid;

use browsertap_shared::codename::generate_unique_codename;
use browsertap_shared::protocol::*;
use browsertap_shared::session::*;
use browsertap_shared::token::*;

/// Pending command waiting for a result from the browser.
pub struct PendingCommand {
    pub sender: oneshot::Sender<CommandResult>,
}

/// Per-session channel for sending daemon messages to the WebSocket writer.
pub type SessionSender = mpsc::UnboundedSender<DaemonMessage>;

/// Shared application state for the daemon.
#[derive(Clone)]
pub struct AppState {
    /// Active sessions keyed by session ID.
    sessions: Arc<DashMap<Uuid, Session>>,
    /// WebSocket senders keyed by session ID.
    ws_senders: Arc<DashMap<Uuid, SessionSender>>,
    /// Pending commands keyed by command ID.
    pending_commands: Arc<DashMap<String, PendingCommand>>,
    /// Shared secret for token signing/verification.
    secret: Arc<Vec<u8>>,
    /// Daemon listen host.
    host: String,
    /// Daemon listen port.
    port: u16,
}

impl AppState {
    pub fn new() -> Result<Self> {
        let secret = load_or_create_secret().context("failed to initialize daemon secret")?;

        let host = std::env::var("BROWSERTAP_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port: u16 = std::env::var("BROWSERTAP_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(4455);

        info!("daemon secret loaded ({} bytes)", secret.len());

        Ok(Self {
            sessions: Arc::new(DashMap::new()),
            ws_senders: Arc::new(DashMap::new()),
            pending_commands: Arc::new(DashMap::new()),
            secret: Arc::new(secret),
            host,
            port,
        })
    }

    pub fn secret(&self) -> &[u8] {
        &self.secret
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    // ─── Session management ──────────────────────────────────────────────

    /// Register a new browser session.
    pub fn register_session(
        &self,
        session_id: Uuid,
        url: String,
        title: String,
        user_agent: String,
        top_origin: String,
        ws_sender: SessionSender,
    ) -> String {
        let existing_names: Vec<String> = self
            .sessions
            .iter()
            .map(|entry| entry.value().codename.clone())
            .collect();

        let codename = generate_unique_codename(&existing_names);

        let session = Session::new(
            session_id,
            codename.clone(),
            url,
            title,
            user_agent,
            top_origin,
        );

        self.sessions.insert(session_id, session);
        self.ws_senders.insert(session_id, ws_sender);

        info!(session_id = %session_id, codename = %codename, "session registered");
        codename
    }

    /// Remove a session.
    pub fn remove_session(&self, session_id: Uuid) {
        self.sessions.remove(&session_id);
        self.ws_senders.remove(&session_id);
        info!(session_id = %session_id, "session removed");
    }

    /// Update heartbeat for a session.
    pub fn heartbeat(&self, session_id: Uuid) {
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            session.touch();
        }
    }

    /// List all active sessions.
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions
            .iter()
            .map(|entry| {
                let s = entry.value();
                SessionInfo {
                    session_id: s.id,
                    codename: s.codename.clone(),
                    url: s.url.clone(),
                    title: s.title.clone(),
                    user_agent: s.user_agent.clone(),
                    socket_state: s.socket_state,
                    connected_at: s.connected_at.timestamp(),
                    last_heartbeat: s.last_heartbeat.timestamp(),
                    console_buffer_size: s.console_buffer.len(),
                    network_buffer_size: s.network_buffer.len(),
                }
            })
            .collect()
    }

    /// Find a session by ID or codename.
    pub fn find_session(&self, id_or_codename: &str) -> Option<SessionInfo> {
        // Try UUID first
        if let Ok(uuid) = id_or_codename.parse::<Uuid>() {
            return self.sessions.get(&uuid).map(|s| session_to_info(s.value()));
        }
        // Then try codename
        self.sessions
            .iter()
            .find(|entry| entry.value().codename == id_or_codename)
            .map(|entry| session_to_info(entry.value()))
    }

    /// Resolve a codename to a session UUID. Used by future middleware.
    #[allow(dead_code)]
    pub fn resolve_session_id(&self, id_or_codename: &str) -> Option<Uuid> {
        if let Ok(uuid) = id_or_codename.parse::<Uuid>()
            && self.sessions.contains_key(&uuid)
        {
            return Some(uuid);
        }
        self.sessions
            .iter()
            .find(|entry| entry.value().codename == id_or_codename)
            .map(|entry| *entry.key())
    }

    // ─── Telemetry ───────────────────────────────────────────────────────

    /// Push console events from browser to session buffer.
    pub fn push_console_events(&self, session_id: Uuid, events: Vec<ConsoleEvent>) {
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            session.push_console_events(events);
        }
    }

    /// Push network events from browser to session buffer.
    pub fn push_network_events(&self, session_id: Uuid, events: Vec<NetworkEvent>) {
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            session.push_network_events(events);
        }
    }

    /// Get console buffer for a session.
    pub fn get_console_buffer(&self, session_id: Uuid) -> Option<Vec<ConsoleEvent>> {
        self.sessions
            .get(&session_id)
            .map(|s| s.console_buffer.clone())
    }

    /// Get network buffer for a session.
    pub fn get_network_buffer(&self, session_id: Uuid) -> Option<Vec<NetworkEvent>> {
        self.sessions
            .get(&session_id)
            .map(|s| s.network_buffer.clone())
    }

    // ─── Command routing ─────────────────────────────────────────────────

    /// Send a command to a browser session and wait for the result.
    pub async fn send_command(
        &self,
        session_id: Uuid,
        command: BrowserCommand,
        timeout_ms: u64,
    ) -> Result<CommandResult, anyhow::Error> {
        let command_id = command.id().to_string();

        let ws_sender = self
            .ws_senders
            .get(&session_id)
            .ok_or_else(|| anyhow::anyhow!("session not connected"))?
            .clone();

        let (tx, rx) = oneshot::channel();
        self.pending_commands
            .insert(command_id.clone(), PendingCommand { sender: tx });

        let msg = DaemonMessage::Command {
            session_id,
            command,
        };
        ws_sender
            .send(msg)
            .map_err(|_| anyhow::anyhow!("WebSocket send failed"))?;

        let result = tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), rx)
            .await
            .map_err(|_| {
                self.pending_commands.remove(&command_id);
                anyhow::anyhow!("command timed out after {timeout_ms}ms")
            })?
            .map_err(|_| anyhow::anyhow!("command channel closed"))?;

        Ok(result)
    }

    /// Resolve a pending command with a result from the browser.
    pub fn resolve_command(&self, command_id: &str, result: CommandResult) {
        if let Some((_, pending)) = self.pending_commands.remove(command_id) {
            let _ = pending.sender.send(result);
        } else {
            warn!(command_id = command_id, "no pending command found");
        }
    }

    // ─── Stale session cleanup ───────────────────────────────────────────

    /// Remove stale sessions. Called by the periodic cleanup task.
    #[allow(dead_code)]
    pub fn cleanup_stale_sessions(&self) {
        let stale_ids: Vec<Uuid> = self
            .sessions
            .iter()
            .filter(|entry| entry.value().is_stale())
            .map(|entry| *entry.key())
            .collect();

        for id in &stale_ids {
            if let Some(sender) = self.ws_senders.get(id) {
                let _ = sender.send(DaemonMessage::Disconnect {
                    reason: "heartbeat timeout".into(),
                });
            }
            self.remove_session(*id);
        }

        if !stale_ids.is_empty() {
            info!(count = stale_ids.len(), "cleaned up stale sessions");
        }
    }

    /// Verify a token from the browser.
    pub fn verify_session_token(
        &self,
        token: &str,
    ) -> Result<browsertap_shared::token::TokenPayload, TokenError> {
        verify_token_with_scope(token, &self.secret, TokenScope::Session)
    }
}

fn session_to_info(s: &Session) -> SessionInfo {
    SessionInfo {
        session_id: s.id,
        codename: s.codename.clone(),
        url: s.url.clone(),
        title: s.title.clone(),
        user_agent: s.user_agent.clone(),
        socket_state: s.socket_state,
        connected_at: s.connected_at.timestamp(),
        last_heartbeat: s.last_heartbeat.timestamp(),
        console_buffer_size: s.console_buffer.len(),
        network_buffer_size: s.network_buffer.len(),
    }
}

/// Create an AppState with a known secret for testing.
#[cfg(test)]
pub fn test_state_with_secret(secret: Vec<u8>) -> AppState {
    AppState {
        sessions: Arc::new(DashMap::new()),
        ws_senders: Arc::new(DashMap::new()),
        pending_commands: Arc::new(DashMap::new()),
        secret: Arc::new(secret),
        host: "127.0.0.1".into(),
        port: 4455,
    }
}

/// Load secret from disk or create a new one.
fn load_or_create_secret() -> Result<Vec<u8>> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let dir = std::path::PathBuf::from(&home).join(".browsertap");
    let path = dir.join("secret.key");

    // Check env var first
    if let Ok(hex) = std::env::var("BROWSERTAP_SECRET") {
        return secret_from_hex(&hex)
            .map_err(|e| anyhow::anyhow!("invalid BROWSERTAP_SECRET: {e}"));
    }

    // Try reading from file
    if path.exists() {
        let hex = std::fs::read_to_string(&path)
            .context("failed to read secret file")?
            .trim()
            .to_string();
        return secret_from_hex(&hex).map_err(|e| anyhow::anyhow!("invalid secret file: {e}"));
    }

    // Generate new secret
    std::fs::create_dir_all(&dir).context("failed to create ~/.browsertap")?;
    let secret = generate_secret();
    let hex = secret_to_hex(&secret);
    std::fs::write(&path, &hex).context("failed to write secret file")?;

    // Set file permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }

    info!("generated new daemon secret at {}", path.display());
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use browsertap_shared::token::generate_secret;

    fn make_state() -> AppState {
        test_state_with_secret(generate_secret())
    }

    #[test]
    fn register_and_find_session() {
        let state = make_state();
        let (tx, _rx) = mpsc::unbounded_channel();
        let session_id = Uuid::new_v4();
        let codename = state.register_session(
            session_id,
            "http://localhost:3000".into(),
            "Dashboard".into(),
            "Mozilla/5.0".into(),
            "http://localhost:3000".into(),
            tx,
        );

        // Find by UUID
        let found = state.find_session(&session_id.to_string());
        assert!(found.is_some());
        assert_eq!(found.unwrap().codename, codename);

        // Find by codename
        let found = state.find_session(&codename);
        assert!(found.is_some());
        assert_eq!(found.unwrap().session_id, session_id);
    }

    #[test]
    fn remove_session() {
        let state = make_state();
        let (tx, _rx) = mpsc::unbounded_channel();
        let session_id = Uuid::new_v4();
        state.register_session(
            session_id,
            "http://localhost".into(),
            "Test".into(),
            "UA".into(),
            "http://localhost".into(),
            tx,
        );

        assert_eq!(state.list_sessions().len(), 1);
        state.remove_session(session_id);
        assert_eq!(state.list_sessions().len(), 0);
    }

    #[test]
    fn heartbeat_updates_session() {
        let state = make_state();
        let (tx, _rx) = mpsc::unbounded_channel();
        let session_id = Uuid::new_v4();
        state.register_session(
            session_id,
            "http://localhost".into(),
            "Test".into(),
            "UA".into(),
            "http://localhost".into(),
            tx,
        );

        let before = state.sessions.get(&session_id).unwrap().last_heartbeat;
        state.heartbeat(session_id);
        let after = state.sessions.get(&session_id).unwrap().last_heartbeat;
        assert!(after >= before);
    }

    #[test]
    fn console_buffer_push_and_retrieve() {
        let state = make_state();
        let (tx, _rx) = mpsc::unbounded_channel();
        let session_id = Uuid::new_v4();
        state.register_session(
            session_id,
            "http://localhost".into(),
            "Test".into(),
            "UA".into(),
            "http://localhost".into(),
            tx,
        );

        let events = vec![ConsoleEvent {
            id: "e1".into(),
            timestamp: 1000,
            level: ConsoleLevel::Log,
            args: vec![serde_json::json!("hello")],
        }];
        state.push_console_events(session_id, events);

        let buf = state.get_console_buffer(session_id).unwrap();
        assert_eq!(buf.len(), 1);
        assert_eq!(buf[0].id, "e1");
    }

    #[test]
    fn network_buffer_push_and_retrieve() {
        let state = make_state();
        let (tx, _rx) = mpsc::unbounded_channel();
        let session_id = Uuid::new_v4();
        state.register_session(
            session_id,
            "http://localhost".into(),
            "Test".into(),
            "UA".into(),
            "http://localhost".into(),
            tx,
        );

        let events = vec![NetworkEvent {
            id: "n1".into(),
            timestamp: 2000,
            method: "GET".into(),
            url: "http://api.example.com".into(),
            status: Some(200),
            duration_ms: Some(42),
            error: None,
        }];
        state.push_network_events(session_id, events);

        let buf = state.get_network_buffer(session_id).unwrap();
        assert_eq!(buf.len(), 1);
        assert_eq!(buf[0].status, Some(200));
    }

    #[test]
    fn resolve_session_id_by_codename() {
        let state = make_state();
        let (tx, _rx) = mpsc::unbounded_channel();
        let session_id = Uuid::new_v4();
        let codename = state.register_session(
            session_id,
            "http://localhost".into(),
            "Test".into(),
            "UA".into(),
            "http://localhost".into(),
            tx,
        );

        assert_eq!(state.resolve_session_id(&codename), Some(session_id));
        assert_eq!(
            state.resolve_session_id(&session_id.to_string()),
            Some(session_id)
        );
        assert_eq!(state.resolve_session_id("nonexistent"), None);
    }

    #[test]
    fn verify_token_roundtrip() {
        let state = make_state();
        let session_id = Uuid::new_v4();
        let payload = TokenPayload::new(TokenScope::Session, "test-app", session_id);
        let token = sign_token(&payload, state.secret()).unwrap();

        let verified = state.verify_session_token(&token);
        assert!(verified.is_ok());
        assert_eq!(verified.unwrap().session_id, session_id);
    }

    #[test]
    fn buffers_for_unknown_session_return_none() {
        let state = make_state();
        let fake_id = Uuid::new_v4();
        assert!(state.get_console_buffer(fake_id).is_none());
        assert!(state.get_network_buffer(fake_id).is_none());
    }
}
