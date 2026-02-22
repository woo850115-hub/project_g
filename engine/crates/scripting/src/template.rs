use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::info;

use crate::error::ScriptError;

/// Metadata from a game template's game.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct GameTemplate {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scripts: Vec<String>,
}

/// Result of loading a game template: the parsed config and resolved script directory.
#[derive(Debug)]
pub struct LoadedTemplate {
    pub config: GameTemplate,
    pub scripts_dir: PathBuf,
    pub base_dir: PathBuf,
}

/// Load a game template from a directory.
///
/// Expected structure:
/// ```text
/// games/my_game/
/// ├── game.toml
/// └── scripts/
///     ├── world_setup.lua
///     └── combat.lua
/// ```
///
/// If `game.toml` has a `scripts` list, only those files are loaded (in order).
/// If `scripts` is empty, all .lua/.luau files in scripts/ are loaded alphabetically.
pub fn load_template(dir: &Path) -> Result<LoadedTemplate, ScriptError> {
    let config_path = dir.join("game.toml");
    if !config_path.exists() {
        return Err(ScriptError::Load(format!(
            "game.toml not found in {}",
            dir.display()
        )));
    }

    let config_str = std::fs::read_to_string(&config_path)?;
    let config: GameTemplate = toml::from_str(&config_str)
        .map_err(|e| ScriptError::Load(format!("failed to parse game.toml: {}", e)))?;

    let scripts_dir = dir.join("scripts");

    info!(
        name = %config.name,
        version = %config.version,
        "Loaded game template"
    );

    Ok(LoadedTemplate {
        config,
        scripts_dir,
        base_dir: dir.to_path_buf(),
    })
}

/// Load scripts from a template into a ScriptEngine.
/// If the template specifies scripts in order, load them in that order.
/// Otherwise, load all .lua/.luau files from the scripts directory.
pub fn load_template_scripts(
    engine: &mut crate::engine::ScriptEngine,
    template: &LoadedTemplate,
) -> Result<(), ScriptError> {
    if !template.scripts_dir.exists() {
        info!("No scripts/ directory in template, skipping script loading");
        return Ok(());
    }

    if template.config.scripts.is_empty() {
        // Load all scripts alphabetically
        engine.load_directory(&template.scripts_dir)?;
    } else {
        // Load scripts in the specified order
        for script_name in &template.config.scripts {
            let mut script_path = template.scripts_dir.join(script_name);

            // Try adding .lua extension if not present
            if !script_path.exists() && script_path.extension().is_none() {
                script_path = template.scripts_dir.join(format!("{}.lua", script_name));
            }

            if !script_path.exists() {
                return Err(ScriptError::Load(format!(
                    "script not found: {}",
                    script_path.display()
                )));
            }

            let source = std::fs::read_to_string(&script_path)?;
            let name = script_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            engine.load_script(name, &source)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::ScriptConfig;

    fn create_test_template(dir: &Path) {
        std::fs::create_dir_all(dir.join("scripts")).unwrap();

        std::fs::write(
            dir.join("game.toml"),
            r#"
name = "Test MUD"
version = "0.1.0"
description = "A test game"
scripts = ["world_setup", "commands"]
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("scripts/world_setup.lua"),
            r#"
hooks.on_tick(function(tick)
    -- setup
end)
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("scripts/commands.lua"),
            r#"
hooks.on_action("dance", function(ctx)
    output:send(ctx.session_id, "You dance!")
    return true
end)
"#,
        )
        .unwrap();
    }

    #[test]
    fn test_load_template() {
        let dir = std::env::temp_dir().join("scripting_test_template");
        let _ = std::fs::remove_dir_all(&dir);
        create_test_template(&dir);

        let template = load_template(&dir).unwrap();
        assert_eq!(template.config.name, "Test MUD");
        assert_eq!(template.config.version, "0.1.0");
        assert_eq!(template.config.scripts.len(), 2);
        assert_eq!(template.config.scripts[0], "world_setup");
        assert_eq!(template.config.scripts[1], "commands");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_template_scripts() {
        let dir = std::env::temp_dir().join("scripting_test_template_scripts");
        let _ = std::fs::remove_dir_all(&dir);
        create_test_template(&dir);

        let template = load_template(&dir).unwrap();
        let mut engine = crate::engine::ScriptEngine::new(ScriptConfig::default()).unwrap();
        load_template_scripts(&mut engine, &template).unwrap();

        assert_eq!(engine.script_count(), 2);
        assert_eq!(engine.hook_registry().on_tick_count(), 1);
        assert_eq!(engine.hook_registry().on_action_count(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_template_no_game_toml() {
        let dir = std::env::temp_dir().join("scripting_test_no_toml");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let result = load_template(&dir);
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_template_auto_discovery() {
        let dir = std::env::temp_dir().join("scripting_test_auto_discovery");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scripts")).unwrap();

        std::fs::write(
            dir.join("game.toml"),
            r#"
name = "Auto MUD"
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("scripts/01_first.lua"),
            "hooks.on_tick(function() end)",
        )
        .unwrap();
        std::fs::write(
            dir.join("scripts/02_second.lua"),
            "hooks.on_tick(function() end)",
        )
        .unwrap();

        let template = load_template(&dir).unwrap();
        assert!(template.config.scripts.is_empty());

        let mut engine = crate::engine::ScriptEngine::new(ScriptConfig::default()).unwrap();
        load_template_scripts(&mut engine, &template).unwrap();

        assert_eq!(engine.script_count(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
