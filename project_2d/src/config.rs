use std::path::Path;

use serde::Deserialize;

use engine_core::tick::TickConfig;
use scripting::ScriptConfig;
use space::grid_space::GridConfig;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NetConfig {
    pub ws_addr: String,
    pub max_connections: usize,
    pub web_static_dir: String,
}

impl Default for NetConfig {
    fn default() -> Self {
        Self {
            ws_addr: "0.0.0.0:4001".to_string(),
            max_connections: 1000,
            web_static_dir: "web_dist".to_string(),
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
pub struct ScriptSection {
    pub scripts_dir: String,
    pub grid_scripts_dir: String,
    pub content_dir: String,
    pub memory_limit_kb: usize,
    pub instruction_limit: u32,
}

impl Default for ScriptSection {
    fn default() -> Self {
        Self {
            scripts_dir: "scripts".to_string(),
            grid_scripts_dir: "scripts_grid".to_string(),
            content_dir: "content".to_string(),
            memory_limit_kb: 16384,       // 16 MB
            instruction_limit: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GridSection {
    pub width: u32,
    pub height: u32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub aoi_radius: u32,
}

impl Default for GridSection {
    fn default() -> Self {
        Self {
            width: 256,
            height: 256,
            origin_x: 0,
            origin_y: 0,
            aoi_radius: 32,
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

/// Top-level Grid server configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub net: NetConfig,
    pub tick: TickSection,
    pub scripting: ScriptSection,
    pub grid: GridSection,
    pub security: SecuritySection,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            net: NetConfig::default(),
            tick: TickSection::default(),
            scripting: ScriptSection::default(),
            grid: GridSection::default(),
            security: SecuritySection::default(),
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

    /// Convert grid section to space crate's GridConfig.
    pub fn to_grid_config(&self) -> GridConfig {
        GridConfig {
            width: self.grid.width,
            height: self.grid.height,
            origin_x: self.grid.origin_x,
            origin_y: self.grid.origin_y,
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
        assert_eq!(config.net.ws_addr, "0.0.0.0:4001");
        assert_eq!(config.tick.tps, 10);
        assert_eq!(config.scripting.scripts_dir, "scripts");
        assert_eq!(config.scripting.grid_scripts_dir, "scripts_grid");
        assert_eq!(config.scripting.content_dir, "content");
        assert_eq!(config.grid.width, 256);
        assert_eq!(config.grid.height, 256);
        assert_eq!(config.grid.aoi_radius, 32);
        assert_eq!(config.security.max_connections_per_ip, 5);
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
    fn to_grid_config() {
        let config = ServerConfig::default();
        let gc = config.to_grid_config();
        assert_eq!(gc.width, 256);
        assert_eq!(gc.height, 256);
        assert_eq!(gc.origin_x, 0);
        assert_eq!(gc.origin_y, 0);
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

[grid]
width = 512
"#).unwrap();

        let config = ServerConfig::load(Some(f.path().to_str().unwrap())).unwrap();
        assert_eq!(config.tick.tps, 20);
        assert_eq!(config.grid.width, 512);
        assert_eq!(config.grid.height, 256);
        assert_eq!(config.net.ws_addr, "0.0.0.0:4001");
    }
}
