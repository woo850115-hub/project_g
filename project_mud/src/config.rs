use std::path::Path;

use serde::Deserialize;

use engine_core::tick::TickConfig;
use scripting::ScriptConfig;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NetConfig {
    pub telnet_addr: String,
    pub max_connections: usize,
}

impl Default for NetConfig {
    fn default() -> Self {
        Self {
            telnet_addr: "0.0.0.0:4000".to_string(),
            max_connections: 1000,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TickSection {
    pub tps: u32,
}

impl Default for TickSection {
    fn default() -> Self {
        Self { tps: 10 }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PersistSection {
    pub snapshot_interval: u64,
    pub save_dir: String,
}

impl Default for PersistSection {
    fn default() -> Self {
        Self {
            snapshot_interval: 300,
            save_dir: "data/snapshots".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ScriptSection {
    pub scripts_dir: String,
    pub content_dir: String,
    pub memory_limit_kb: usize,
    pub instruction_limit: u32,
}

impl Default for ScriptSection {
    fn default() -> Self {
        Self {
            scripts_dir: "scripts".to_string(),
            content_dir: "content".to_string(),
            memory_limit_kb: 16384,       // 16 MB
            instruction_limit: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DatabaseSection {
    pub path: String,
    pub auth_required: bool,
}

impl Default for DatabaseSection {
    fn default() -> Self {
        Self {
            path: "data/player.db".to_string(),
            auth_required: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SecuritySection {
    pub max_connections_total: usize,
    pub max_connections_per_ip: usize,
    pub max_commands_per_second: u32,
    pub max_input_length: usize,
}

impl Default for SecuritySection {
    fn default() -> Self {
        Self {
            max_connections_total: 1000,
            max_connections_per_ip: 5,
            max_commands_per_second: 20,
            max_input_length: 4096,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CharacterSection {
    pub save_interval: u64,
    pub linger_timeout_secs: u64,
}

impl Default for CharacterSection {
    fn default() -> Self {
        Self {
            save_interval: 600,       // 600 ticks = 60 seconds at 10 TPS
            linger_timeout_secs: 60,
        }
    }
}

/// Top-level MUD server configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub net: NetConfig,
    pub tick: TickSection,
    pub persistence: PersistSection,
    pub scripting: ScriptSection,
    pub database: DatabaseSection,
    pub security: SecuritySection,
    pub character: CharacterSection,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            net: NetConfig::default(),
            tick: TickSection::default(),
            persistence: PersistSection::default(),
            scripting: ScriptSection::default(),
            database: DatabaseSection::default(),
            security: SecuritySection::default(),
            character: CharacterSection::default(),
        }
    }
}

impl ServerConfig {
    /// Load configuration from an optional TOML file path.
    pub fn load(config_path: Option<&str>) -> Result<Self, Box<dyn std::error::Error>> {
        let config = match config_path {
            Some(path) if Path::new(path).exists() => {
                let content = std::fs::read_to_string(path)?;
                toml::from_str(&content)?
            }
            _ => Self::default(),
        };
        Ok(config)
    }

    /// Convert tick section to engine_core's TickConfig.
    pub fn to_tick_config(&self) -> TickConfig {
        TickConfig {
            tps: self.tick.tps,
            max_ticks: 0,
        }
    }

    /// Convert scripting section to scripting crate's ScriptConfig.
    pub fn to_script_config(&self) -> ScriptConfig {
        ScriptConfig {
            memory_limit: self.scripting.memory_limit_kb * 1024,
            instruction_limit: self.scripting.instruction_limit,
        }
    }
}

/// Parse CLI arguments and load config.
/// Supports: --config <path>
pub fn parse_cli_args() -> ServerConfig {
    let args: Vec<String> = std::env::args().collect();
    let mut config_path: Option<&str> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                if let Some(val) = args.get(i + 1) {
                    config_path = Some(val.as_str());
                    i += 2;
                } else {
                    eprintln!("--config requires a path argument");
                    std::process::exit(1);
                }
            }
            other => {
                eprintln!("Unknown argument: {}", other);
                std::process::exit(1);
            }
        }
    }

    match ServerConfig::load(config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn default_config_matches_hardcoded_values() {
        let config = ServerConfig::default();
        assert_eq!(config.net.telnet_addr, "0.0.0.0:4000");
        assert_eq!(config.tick.tps, 10);
        assert_eq!(config.persistence.snapshot_interval, 300);
        assert_eq!(config.persistence.save_dir, "data/snapshots");
        assert_eq!(config.scripting.scripts_dir, "scripts");
        assert_eq!(config.scripting.content_dir, "content");
        assert_eq!(config.security.max_connections_per_ip, 5);
        assert_eq!(config.security.max_commands_per_second, 20);
    }

    #[test]
    fn to_tick_config() {
        let config = ServerConfig::default();
        let tc = config.to_tick_config();
        assert_eq!(tc.tps, 10);
        assert_eq!(tc.max_ticks, 0);
    }

    #[test]
    fn to_script_config() {
        let config = ServerConfig::default();
        let sc = config.to_script_config();
        assert_eq!(sc.memory_limit, 16384 * 1024);
        assert_eq!(sc.instruction_limit, 1_000_000);
    }

    #[test]
    fn load_nonexistent_file_returns_defaults() {
        let config = ServerConfig::load(Some("/tmp/nonexistent_config_12345.toml")).unwrap();
        assert_eq!(config.tick.tps, 10);
    }

    #[test]
    fn load_none_returns_defaults() {
        let config = ServerConfig::load(None).unwrap();
        assert_eq!(config.tick.tps, 10);
    }

    #[test]
    fn load_partial_toml() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, r#"
[tick]
tps = 20
"#).unwrap();

        let config = ServerConfig::load(Some(f.path().to_str().unwrap())).unwrap();
        assert_eq!(config.tick.tps, 20);
        assert_eq!(config.net.telnet_addr, "0.0.0.0:4000");
    }
}
