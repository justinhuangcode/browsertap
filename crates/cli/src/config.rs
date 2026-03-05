use anyhow::{Context, Result};

use browsertap_shared::session::ProjectConfig;

/// Load project configuration from browsertap.toml, walking up directories.
pub fn load_config() -> Result<ProjectConfig> {
    let mut dir = std::env::current_dir().context("failed to get current directory")?;

    loop {
        let config_path = dir.join("browsertap.toml");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("failed to read {}", config_path.display()))?;
            let config: ProjectConfig = toml::from_str(&content)
                .with_context(|| format!("failed to parse {}", config_path.display()))?;
            return Ok(config);
        }

        if !dir.pop() {
            break;
        }
    }

    // No config file found, return defaults
    Ok(ProjectConfig::default())
}

#[cfg(test)]
mod tests {
    use browsertap_shared::session::ProjectConfig;

    #[test]
    fn parse_minimal_config() {
        let toml_str = r#"
app_label = "TestApp"
app_url = "http://localhost:3000"
"#;
        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.app_label.as_deref(), Some("TestApp"));
        assert_eq!(config.app_url.as_deref(), Some("http://localhost:3000"));
        assert_eq!(config.daemon.port, 4455); // default
    }

    #[test]
    fn parse_full_config() {
        let toml_str = r#"
app_label = "MyApp"
app_url = "http://localhost:8080"
daemon_url = "https://127.0.0.1:9999"

[daemon]
host = "0.0.0.0"
port = 9999

[smoke]
defaults = ["home", "about"]

[smoke.presets]
full = ["home", "about", "contact", "pricing"]
quick = ["home"]

[smoke.redirects]
"/" = "/home"
"#;
        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.daemon.host, "0.0.0.0");
        assert_eq!(config.daemon.port, 9999);
        assert_eq!(config.smoke.defaults, vec!["home", "about"]);
        assert_eq!(config.smoke.presets.len(), 2);
        assert_eq!(config.smoke.presets["full"].len(), 4);
        assert_eq!(config.smoke.redirects["/"], "/home");
    }

    #[test]
    fn empty_config_uses_defaults() {
        let config: ProjectConfig = toml::from_str("").unwrap();
        assert!(config.app_label.is_none());
        assert!(config.app_url.is_none());
        assert_eq!(config.daemon.host, "127.0.0.1");
        assert_eq!(config.daemon.port, 4455);
        assert!(config.smoke.defaults.is_empty());
    }
}
