use crate::database::{ConnectionConfig, SSHConfig};
use crate::input::KeyConfig;
use crate::theme::Theme;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize, Serialize)]
pub struct ConfigFile {
    pub theme: String,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub connections: Vec<ConnectionConfig>,
    #[serde(default)]
    pub ssh_tunnels: Vec<SSHTunnelProfile>,
    #[serde(default)]
    pub keymap: KeyConfig,
}

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct DatabaseConfig {
    pub default_port_postgres: u16,
    pub default_port_mongodb: u16,
}

#[derive(Clone)]
pub struct Config {
    pub theme_name: String,
    pub theme: Theme,
    pub database: DatabaseConfig,
    pub connections: Vec<ConnectionConfig>,
    pub ssh_tunnels: Vec<SSHTunnelProfile>,
    pub keymap: KeyConfig,
}

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct SSHTunnelProfile {
    pub name: String,
    #[serde(flatten)]
    pub config: SSHConfig,
}

impl Config {
    fn get_config_dir() -> PathBuf {
        // Get the home directory
        let home = std::env::var("HOME").expect("Could not find HOME directory");
        let config_dir = PathBuf::from(home).join(".config").join("lazylode");

        config_dir
    }

    fn load_theme(theme_name: &str) -> Result<Theme> {
        let theme_dir = Self::get_config_dir().join("themes");
        let theme_path = theme_dir.join(format!("{}.toml", theme_name));

        if theme_path.exists() {
            let content =
                std::fs::read_to_string(&theme_path).context("Failed to read theme file")?;
            toml::from_str(&content).context("Failed to parse theme file")
        } else {
            Ok(Theme::default())
        }
    }

    fn load_config() -> Result<ConfigFile> {
        let config_dir = Self::get_config_dir();
        let config_path = config_dir.join("config.toml");

        if config_path.exists() {
            let content =
                std::fs::read_to_string(&config_path).context("Failed to read config file")?;
            toml::from_str(&content).context("Failed to parse config file")
        } else {
            std::fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

            let default_config = ConfigFile {
                theme: String::from("catppuccin_mocha"),
                database: DatabaseConfig {
                    default_port_postgres: 5432,
                    default_port_mongodb: 27017,
                },
                connections: Vec::new(),
                ssh_tunnels: Vec::new(),
                keymap: KeyConfig::default(),
            };

            let toml_string = toml::to_string_pretty(&default_config)
                .context("Failed to serialize default config")?;

            std::fs::write(&config_path, toml_string).context("Failed to write config file")?;

            Ok(default_config)
        }
    }

    pub fn new() -> Self {
        let config_file = Self::load_config().unwrap_or_else(|err| {
            eprintln!("Error loading config: {}", err);
            ConfigFile {
                theme: String::from("default"),
                database: DatabaseConfig::default(),
                connections: Vec::new(),
                ssh_tunnels: Vec::new(),
                keymap: KeyConfig::default(),
            }
        });

        let theme = Self::load_theme(&config_file.theme).unwrap_or_else(|err| {
            eprintln!("Error loading theme: {}", err);
            Theme::default()
        });

        Self {
            theme,
            theme_name: config_file.theme,
            database: config_file.database,
            connections: config_file.connections,
            ssh_tunnels: config_file.ssh_tunnels,
            keymap: config_file.keymap,
        }
    }

    // Save connections to config file
    pub fn save_connections(&self, connections: &Vec<ConnectionConfig>) -> Result<()> {
        let config_dir = Self::get_config_dir();
        let config_path = config_dir.join("config.toml");

        let mut config_file = Self::load_config()?;
        config_file.connections = connections.clone();

        let toml_string = toml::to_string_pretty(&config_file)
            .context("Failed to serialize config with connections")?;

        std::fs::write(&config_path, toml_string)
            .context("Failed to write config file with connections")?;

        Ok(())
    }

    // Load connections from config file
    pub fn load_connections(&self) -> Result<Vec<ConnectionConfig>> {
        let config_file = Self::load_config()?;
        Ok(config_file.connections)
    }

    // Save entire configuration
    pub fn save(&self) -> Result<()> {
        let config_dir = Self::get_config_dir();
        let config_path = config_dir.join("config.toml");

        let config_file = ConfigFile {
            theme: self.theme_name.clone(),
            database: self.database.clone(),
            connections: self.connections.clone(),
            ssh_tunnels: self.ssh_tunnels.clone(),
            keymap: self.keymap.clone(),
        };

        let toml_string =
            toml::to_string_pretty(&config_file).context("Failed to serialize config")?;

        std::fs::write(&config_path, toml_string).context("Failed to write config file")?;

        Ok(())
    }
}
