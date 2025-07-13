use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub paths: PathConfig,
    pub server: ServerConfig,
    pub syntax_highlighting: SyntaxConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PathConfig {
    pub pages: PathBuf,
    pub components: PathBuf,
    pub themes: PathBuf,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SyntaxConfig {
    pub default_theme: String,
    pub load_custom_themes: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            paths: PathConfig {
                pages: PathBuf::from("/var/www/htmlua/pages"),
                components: PathBuf::from("/var/www/htmlua/components"),
                themes: PathBuf::from("/var/www/htmlua/themes"),
            },
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
            },
            syntax_highlighting: SyntaxConfig {
                default_theme: "base16-ocean.dark".to_string(),
                load_custom_themes: true,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path();
        if config_path.exists() {
            let config_content = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
            let config: Config = toml::from_str(&config_content)
                .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;
            Ok(config)
        } else {
            let default_config = Config::default();
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
            }
            let config_content =
                toml::to_string_pretty(&default_config).context("Failed to serialize default config")?;
            fs::write(&config_path, config_content)
                .with_context(|| format!("Failed to write default config to: {}", config_path.display()))?;
            Ok(default_config)
        }
    }

    fn get_config_path() -> PathBuf {
        if cfg!(windows) {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
                .join("htmlua")
                .join("config.toml")
        } else {
            PathBuf::from("/etc/htmlua.toml")
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path();
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }
        let config_content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(&config_path, config_content)
            .with_context(|| format!("Failed to write config to: {}", config_path.display()))?;
        Ok(())
    }

    pub fn config_file_path() -> PathBuf { Self::get_config_path() }
}
