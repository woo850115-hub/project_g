use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct MudMakerConfig {
    #[serde(default = "default_server")]
    pub server: ServerSection,
    #[serde(default = "default_project")]
    pub project: ProjectSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSection {
    #[serde(default = "default_addr")]
    pub addr: String,
    #[serde(default = "default_web_static_dir")]
    pub web_static_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSection {
    #[serde(default = "default_mud_dir")]
    pub mud_dir: String,
    #[serde(default = "default_mud_config")]
    pub mud_config: String,
    #[serde(default = "default_telnet_addr")]
    pub telnet_addr: String,
}

fn default_server() -> ServerSection {
    ServerSection {
        addr: default_addr(),
        web_static_dir: default_web_static_dir(),
    }
}

fn default_project() -> ProjectSection {
    ProjectSection {
        mud_dir: default_mud_dir(),
        mud_config: default_mud_config(),
        telnet_addr: default_telnet_addr(),
    }
}

fn default_telnet_addr() -> String {
    "127.0.0.1:4000".to_string()
}

fn default_addr() -> String {
    "0.0.0.0:3000".to_string()
}

fn default_web_static_dir() -> String {
    "project_mud_maker/web_dist".to_string()
}

fn default_mud_dir() -> String {
    "project_mud".to_string()
}

fn default_mud_config() -> String {
    "project_mud/server.toml".to_string()
}

impl MudMakerConfig {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let text = std::fs::read_to_string(path)?;
        let config: MudMakerConfig = toml::from_str(&text)?;
        Ok(config)
    }

    pub fn content_dir(&self) -> PathBuf {
        PathBuf::from(&self.project.mud_dir).join("content")
    }

    pub fn scripts_dir(&self) -> PathBuf {
        PathBuf::from(&self.project.mud_dir).join("scripts")
    }
}
