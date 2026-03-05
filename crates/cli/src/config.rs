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
