use anyhow::{Context, Result};
use uuid::Uuid;

use browsertap_shared::protocol::*;

/// HTTP client for communicating with the browsertap daemon.
pub struct DaemonClient {
    base_url: String,
    http: reqwest::Client,
}

impl DaemonClient {
    pub fn new(daemon_url: &str) -> Result<Self> {
        // Accept self-signed certs for local development
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .context("failed to create HTTP client")?;

        Ok(Self {
            base_url: daemon_url.trim_end_matches('/').to_string(),
            http,
        })
    }

    /// List all active sessions.
    pub async fn list_sessions(&self) -> Result<Vec<SessionInfo>> {
        let resp = self
            .http
            .get(format!("{}/api/sessions", self.base_url))
            .send()
            .await
            .context("failed to reach daemon")?;

        resp.json().await.context("failed to parse sessions")
    }

    /// Find a session by codename or ID.
    pub async fn find_session(&self, id_or_codename: &str) -> Result<SessionInfo> {
        let resp = self
            .http
            .get(format!("{}/api/sessions/{}", self.base_url, id_or_codename))
            .send()
            .await
            .context("failed to reach daemon")?;

        if resp.status() == 404 {
            anyhow::bail!("session '{id_or_codename}' not found");
        }

        resp.json().await.context("failed to parse session")
    }

    /// Send a command to a session and wait for result.
    async fn send_command(
        &self,
        session: &str,
        command: BrowserCommand,
        timeout_ms: u64,
    ) -> Result<CommandResult> {
        // Resolve codename to session ID
        let info = self.find_session(session).await?;

        let resp = self
            .http
            .post(format!(
                "{}/api/sessions/{}/command",
                self.base_url, info.session_id
            ))
            .json(&CommandRequest {
                command,
                timeout_ms,
            })
            .send()
            .await
            .context("failed to send command")?;

        let resp: CommandResponse = resp
            .json()
            .await
            .context("failed to parse command response")?;
        Ok(resp.result)
    }

    /// Execute JavaScript in a session.
    pub async fn run_js(&self, session: &str, code: &str) -> Result<CommandResult> {
        self.send_command(
            session,
            BrowserCommand::RunScript {
                id: Uuid::new_v4().to_string(),
                code: code.to_string(),
                capture_console: false,
            },
            30_000,
        )
        .await
    }

    /// Take a screenshot.
    pub async fn screenshot(
        &self,
        session: &str,
        selector: Option<&str>,
        quality: f32,
    ) -> Result<CommandResult> {
        self.send_command(
            session,
            BrowserCommand::Screenshot {
                id: Uuid::new_v4().to_string(),
                selector: selector.map(String::from),
                quality,
                hooks: Vec::new(),
            },
            30_000,
        )
        .await
    }

    /// Click an element.
    pub async fn click(&self, session: &str, selector: &str) -> Result<CommandResult> {
        self.send_command(
            session,
            BrowserCommand::Click {
                id: Uuid::new_v4().to_string(),
                selector: selector.to_string(),
            },
            30_000,
        )
        .await
    }

    /// Navigate to a URL.
    pub async fn navigate(&self, session: &str, url: &str) -> Result<CommandResult> {
        self.send_command(
            session,
            BrowserCommand::Navigate {
                id: Uuid::new_v4().to_string(),
                url: url.to_string(),
            },
            30_000,
        )
        .await
    }

    /// Discover interactive selectors.
    pub async fn discover_selectors(&self, session: &str) -> Result<CommandResult> {
        self.send_command(
            session,
            BrowserCommand::DiscoverSelectors {
                id: Uuid::new_v4().to_string(),
            },
            30_000,
        )
        .await
    }

    /// Get console events from a session.
    pub async fn get_console(&self, session: &str) -> Result<Vec<ConsoleEvent>> {
        let info = self.find_session(session).await?;
        let resp = self
            .http
            .get(format!(
                "{}/api/sessions/{}/console",
                self.base_url, info.session_id
            ))
            .send()
            .await
            .context("failed to fetch console")?;

        resp.json().await.context("failed to parse console events")
    }
}
