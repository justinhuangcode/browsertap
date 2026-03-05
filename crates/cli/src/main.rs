mod client;
mod config;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "browsertap",
    about = "Tap into your live browser. Close the agent loop.",
    version,
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Daemon URL (default: https://127.0.0.1:4455)
    #[arg(long, global = true, env = "BROWSERTAP_DAEMON_URL")]
    daemon_url: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the browsertap daemon
    Daemon {
        /// Listen host
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Listen port
        #[arg(long, default_value = "4455")]
        port: u16,
    },

    /// List active browser sessions
    Sessions,

    /// Execute JavaScript in a browser session
    #[command(name = "run-js")]
    RunJs {
        /// Session codename or ID
        session: String,
        /// JavaScript code to execute
        code: String,
    },

    /// Take a screenshot of a browser session
    Screenshot {
        /// Session codename or ID
        session: String,
        /// CSS selector of element to capture (full page if omitted)
        #[arg(long, short)]
        selector: Option<String>,
        /// Output file path
        #[arg(long, short, default_value = "screenshot.jpg")]
        output: String,
        /// JPEG quality (0.0 - 1.0)
        #[arg(long, default_value = "0.85")]
        quality: f32,
    },

    /// Click an element in a browser session
    Click {
        /// Session codename or ID
        session: String,
        /// CSS selector of element to click
        selector: String,
    },

    /// Navigate a browser session to a URL
    Navigate {
        /// Session codename or ID
        session: String,
        /// URL to navigate to
        url: String,
    },

    /// Run smoke tests across configured routes
    Smoke {
        /// Session codename or ID
        session: String,
        /// Route preset name
        #[arg(long)]
        preset: Option<String>,
        /// Specific routes (comma-separated)
        #[arg(long)]
        routes: Option<String>,
        /// Number of parallel workers
        #[arg(long, default_value = "1")]
        parallel: usize,
    },

    /// View console logs from a browser session
    Console {
        /// Session codename or ID
        session: String,
        /// Number of recent events to show
        #[arg(long, short, default_value = "50")]
        tail: usize,
        /// Filter by level (log, info, warn, error)
        #[arg(long)]
        level: Option<String>,
    },

    /// Discover interactive selectors on the page
    Selectors {
        /// Session codename or ID
        session: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "browsertap=info".into()),
        )
        .init();

    let cli = Cli::parse();
    let daemon_url = cli
        .daemon_url
        .unwrap_or_else(|| "https://127.0.0.1:4455".into());

    let client = client::DaemonClient::new(&daemon_url)?;

    match cli.command {
        Commands::Daemon { host, port } => {
            eprintln!("Starting browsertap daemon on {host}:{port}...");
            eprintln!("Run: BROWSERTAP_HOST={host} BROWSERTAP_PORT={port} browsertapd");
            Ok(())
        }

        Commands::Sessions => {
            let sessions = client.list_sessions().await?;
            if sessions.is_empty() {
                println!("No active sessions.");
                return Ok(());
            }
            println!(
                "{:<20} {:<40} {:<10} {:<12}",
                "CODENAME", "URL", "STATE", "HEARTBEAT"
            );
            for s in &sessions {
                let ago = chrono::Utc::now().timestamp() - s.last_heartbeat;
                println!(
                    "{:<20} {:<40} {:<10} {:<12}",
                    s.codename,
                    truncate(&s.url, 38),
                    format!("{:?}", s.socket_state),
                    format!("{ago}s ago"),
                );
            }
            println!("\n{} session(s) active.", sessions.len());
            Ok(())
        }

        Commands::RunJs { session, code } => {
            let result = client.run_js(&session, &code).await?;
            if result.ok {
                if let Some(data) = result.data {
                    println!("{}", serde_json::to_string_pretty(&data)?);
                }
            } else if let Some(err) = result.error {
                eprintln!("Error: {err}");
                std::process::exit(1);
            }
            Ok(())
        }

        Commands::Screenshot {
            session,
            selector,
            output,
            quality,
        } => {
            let result = client.screenshot(&session, selector.as_deref(), quality).await?;
            if result.ok {
                if let Some(data) = &result.data {
                    if let Some(b64) = data.get("base64").and_then(|v| v.as_str()) {
                        let bytes = base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            b64,
                        )?;
                        std::fs::write(&output, &bytes)?;
                        println!("Screenshot saved to {output} ({} bytes)", bytes.len());
                    }
                }
            } else if let Some(err) = result.error {
                eprintln!("Screenshot failed: {err}");
                std::process::exit(1);
            }
            Ok(())
        }

        Commands::Click { session, selector } => {
            let result = client.click(&session, &selector).await?;
            if result.ok {
                println!("Clicked: {selector}");
            } else if let Some(err) = result.error {
                eprintln!("Click failed: {err}");
                std::process::exit(1);
            }
            Ok(())
        }

        Commands::Navigate { session, url } => {
            let result = client.navigate(&session, &url).await?;
            if result.ok {
                println!("Navigated to: {url}");
            } else if let Some(err) = result.error {
                eprintln!("Navigate failed: {err}");
                std::process::exit(1);
            }
            Ok(())
        }

        Commands::Smoke {
            session,
            preset,
            routes,
            parallel,
        } => {
            let route_list: Vec<String> = if let Some(routes) = routes {
                routes.split(',').map(|s| s.trim().to_string()).collect()
            } else {
                let preset_name = preset.as_deref().unwrap_or("defaults");
                let config = config::load_config()?;
                config
                    .smoke
                    .presets
                    .get(preset_name)
                    .cloned()
                    .unwrap_or(config.smoke.defaults)
            };

            if route_list.is_empty() {
                eprintln!("No routes configured. Use --routes or configure browsertap.toml");
                return Ok(());
            }

            println!(
                "Smoke testing {} routes (parallel: {parallel})...\n",
                route_list.len()
            );

            let mut pass = 0;
            let mut fail = 0;

            for (i, route) in route_list.iter().enumerate() {
                print!("[{}/{}] /{route} ", i + 1, route_list.len());

                // Navigate to route
                let nav = client.navigate(&session, route).await;
                match nav {
                    Ok(r) if r.ok => {
                        // Check console for errors
                        let console = client.get_console(&session).await.unwrap_or_default();
                        let errors: Vec<_> = console
                            .iter()
                            .filter(|e| {
                                matches!(
                                    e.level,
                                    browsertap_shared::protocol::ConsoleLevel::Error
                                )
                            })
                            .collect();

                        if errors.is_empty() {
                            println!("OK");
                            pass += 1;
                        } else {
                            println!("WARN ({} console errors)", errors.len());
                            for e in &errors {
                                eprintln!("  [error] {:?}", e.args);
                            }
                            pass += 1; // Warnings still pass
                        }
                    }
                    _ => {
                        println!("FAIL");
                        fail += 1;
                    }
                }
            }

            println!("\nResults: {pass} passed, {fail} failed");
            if fail > 0 {
                std::process::exit(1);
            }
            Ok(())
        }

        Commands::Console {
            session,
            tail,
            level,
        } => {
            let events = client.get_console(&session).await?;
            let filtered: Vec<_> = events
                .into_iter()
                .filter(|e| {
                    level.as_ref().is_none_or(|l| {
                        format!("{:?}", e.level).to_lowercase() == l.to_lowercase()
                    })
                })
                .collect();

            let start = filtered.len().saturating_sub(tail);
            for event in &filtered[start..] {
                let ts = chrono::DateTime::from_timestamp(event.timestamp / 1000, 0)
                    .map(|dt| dt.format("%H:%M:%S").to_string())
                    .unwrap_or_else(|| "?".into());
                println!("[{ts}] [{:?}] {:?}", event.level, event.args);
            }
            Ok(())
        }

        Commands::Selectors { session } => {
            let result = client.discover_selectors(&session).await?;
            if result.ok {
                if let Some(data) = result.data {
                    println!("{}", serde_json::to_string_pretty(&data)?);
                }
            }
            Ok(())
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}
