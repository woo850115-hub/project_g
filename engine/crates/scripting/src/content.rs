use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;
use tracing::warn;

use crate::error::ScriptError;

/// Engine-level content registry. Schema-agnostic (no MonsterDef, ItemDef, etc.).
/// Loads JSON files into BTreeMap<collection_name, BTreeMap<id, Value>>.
#[derive(Debug)]
pub struct ContentRegistry {
    collections: BTreeMap<String, BTreeMap<String, Value>>,
}

impl ContentRegistry {
    pub fn new() -> Self {
        Self {
            collections: BTreeMap::new(),
        }
    }

    /// Load all content from a directory.
    /// - Top-level *.json files: parsed as JSON array of objects, each with "id" field
    /// - Subdirectories: each *.json file is a single object with "id" field
    pub fn load_dir(path: &Path) -> Result<Self, ScriptError> {
        let mut registry = Self::new();

        if !path.is_dir() {
            return Err(ScriptError::ContentLoad(format!(
                "not a directory: {}",
                path.display()
            )));
        }

        let mut entries: Vec<_> = std::fs::read_dir(path)
            .map_err(|e| ScriptError::ContentLoad(format!("{}: {}", path.display(), e)))?
            .filter_map(|e| e.ok())
            .collect();

        // Sort for deterministic load order
        entries.sort_by_key(|e| e.file_name());

        if entries.is_empty() {
            warn!("Content directory is empty: {}", path.display());
        }

        for entry in entries {
            let entry_path = entry.path();

            if entry_path.is_dir() {
                // Subdirectory: each *.json file is a single object
                let dir_name = entry_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                registry.load_object_dir(&dir_name, &entry_path)?;
            } else if entry_path
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
            {
                // Top-level *.json file: parsed as array
                let collection = entry_path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                registry.load_array_file(&collection, &entry_path)?;
            }
            // Non-json files are silently ignored
        }

        Ok(registry)
    }

    /// Load a single JSON array file (e.g., monsters.json).
    /// Each element must be an object with an "id" field (string).
    fn load_array_file(&mut self, collection: &str, path: &Path) -> Result<(), ScriptError> {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let content = std::fs::read_to_string(path)
            .map_err(|e| ScriptError::ContentLoad(format!("{}: {}", file_name, e)))?;

        let parsed: Value = serde_json::from_str(&content)
            .map_err(|e| ScriptError::ContentLoad(format!("{}: {}", file_name, e)))?;

        let arr = parsed.as_array().ok_or_else(|| {
            ScriptError::ContentLoad(format!("{}: expected JSON array at top level", file_name))
        })?;

        let col = self
            .collections
            .entry(collection.to_string())
            .or_insert_with(BTreeMap::new);

        for (i, item) in arr.iter().enumerate() {
            let obj = item.as_object().ok_or_else(|| {
                ScriptError::ContentLoad(format!("{}[{}]: expected JSON object", file_name, i))
            })?;

            let id = obj
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ScriptError::ContentLoad(format!(
                        "{}[{}]: missing or non-string 'id' field",
                        file_name, i
                    ))
                })?
                .to_string();

            if col.contains_key(&id) {
                return Err(ScriptError::ContentLoad(format!(
                    "{}: duplicate id '{}'",
                    file_name, id
                )));
            }

            col.insert(id, item.clone());
        }

        Ok(())
    }

    /// Load a directory where each *.json file is a single object with "id" field.
    fn load_object_dir(&mut self, collection: &str, dir: &Path) -> Result<(), ScriptError> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| ScriptError::ContentLoad(format!("{}: {}", dir.display(), e)))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "json")
                    .unwrap_or(false)
            })
            .collect();

        // Sort for deterministic load order
        entries.sort_by_key(|e| e.file_name());

        if entries.is_empty() {
            warn!(
                "Content subdirectory is empty: {}/{}",
                dir.display(),
                collection
            );
            return Ok(());
        }

        let col = self
            .collections
            .entry(collection.to_string())
            .or_insert_with(BTreeMap::new);

        for entry in entries {
            let file_path = entry.path();
            let file_name = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let content = std::fs::read_to_string(&file_path)
                .map_err(|e| ScriptError::ContentLoad(format!("{}: {}", file_name, e)))?;

            let parsed: Value = serde_json::from_str(&content)
                .map_err(|e| ScriptError::ContentLoad(format!("{}: {}", file_name, e)))?;

            if !parsed.is_object() {
                return Err(ScriptError::ContentLoad(format!(
                    "{}: expected JSON object",
                    file_name
                )));
            }

            let id = parsed
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ScriptError::ContentLoad(format!(
                        "{}: missing or non-string 'id' field",
                        file_name
                    ))
                })?
                .to_string();

            if col.contains_key(&id) {
                return Err(ScriptError::ContentLoad(format!(
                    "{}: duplicate id '{}'",
                    file_name, id
                )));
            }

            col.insert(id, parsed);
        }

        Ok(())
    }

    /// Get a single item by collection and id.
    pub fn get(&self, collection: &str, id: &str) -> Option<&Value> {
        self.collections.get(collection)?.get(id)
    }

    /// Get all items in a collection.
    pub fn all(&self, collection: &str) -> Option<&BTreeMap<String, Value>> {
        self.collections.get(collection)
    }

    /// Get all collections.
    pub fn collections(&self) -> &BTreeMap<String, BTreeMap<String, Value>> {
        &self.collections
    }

    /// Get collection names.
    pub fn collection_names(&self) -> Vec<&str> {
        self.collections.keys().map(|s| s.as_str()).collect()
    }

    /// Total number of items across all collections (for logging).
    pub fn total_count(&self) -> usize {
        self.collections.values().map(|c| c.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("content_registry_test_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_load_empty_dir() {
        let dir = make_temp_dir("empty");
        let registry = ContentRegistry::load_dir(&dir).unwrap();
        assert_eq!(registry.total_count(), 0);
        assert!(registry.collection_names().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_array_file() {
        let dir = make_temp_dir("array");
        let json = r#"[
            {"id": "goblin", "name": "Goblin", "hp": 30},
            {"id": "orc", "name": "Orc", "hp": 80}
        ]"#;
        fs::write(dir.join("monsters.json"), json).unwrap();

        let registry = ContentRegistry::load_dir(&dir).unwrap();
        assert_eq!(registry.total_count(), 2);
        assert_eq!(registry.collection_names(), vec!["monsters"]);

        let goblin = registry.get("monsters", "goblin").unwrap();
        assert_eq!(goblin["name"], "Goblin");
        assert_eq!(goblin["hp"], 30);

        let all = registry.all("monsters").unwrap();
        assert_eq!(all.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_object_dir() {
        let dir = make_temp_dir("objdir");
        let zones_dir = dir.join("zones");
        fs::create_dir_all(&zones_dir).unwrap();

        fs::write(
            zones_dir.join("forest.json"),
            r#"{"id": "forest", "name": "Dark Forest", "level": 5}"#,
        )
        .unwrap();
        fs::write(
            zones_dir.join("cave.json"),
            r#"{"id": "cave", "name": "Crystal Cave", "level": 10}"#,
        )
        .unwrap();

        let registry = ContentRegistry::load_dir(&dir).unwrap();
        assert_eq!(registry.total_count(), 2);
        assert_eq!(registry.collection_names(), vec!["zones"]);

        let forest = registry.get("zones", "forest").unwrap();
        assert_eq!(forest["name"], "Dark Forest");
        assert_eq!(forest["level"], 5);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_missing_id_field() {
        let dir = make_temp_dir("no_id");
        let json = r#"[{"name": "Goblin", "hp": 30}]"#;
        fs::write(dir.join("monsters.json"), json).unwrap();

        let result = ContentRegistry::load_dir(&dir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("missing"), "error: {}", err);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_duplicate_id() {
        let dir = make_temp_dir("dup_id");
        let json = r#"[
            {"id": "goblin", "name": "Goblin"},
            {"id": "goblin", "name": "Goblin 2"}
        ]"#;
        fs::write(dir.join("monsters.json"), json).unwrap();

        let result = ContentRegistry::load_dir(&dir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("duplicate"), "error: {}", err);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_non_json_ignored() {
        let dir = make_temp_dir("non_json");
        fs::write(dir.join("readme.txt"), "not json").unwrap();
        fs::write(dir.join("notes.md"), "# notes").unwrap();
        let json = r#"[{"id": "sword", "name": "Sword"}]"#;
        fs::write(dir.join("items.json"), json).unwrap();

        let registry = ContentRegistry::load_dir(&dir).unwrap();
        assert_eq!(registry.total_count(), 1);
        assert_eq!(registry.collection_names(), vec!["items"]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_and_all() {
        let dir = make_temp_dir("get_all");
        let json = r#"[
            {"id": "potion", "name": "Health Potion", "heal": 50},
            {"id": "elixir", "name": "Elixir", "heal": 100}
        ]"#;
        fs::write(dir.join("items.json"), json).unwrap();

        let registry = ContentRegistry::load_dir(&dir).unwrap();

        // get existing
        assert!(registry.get("items", "potion").is_some());
        assert_eq!(registry.get("items", "potion").unwrap()["heal"], 50);

        // get non-existing
        assert!(registry.get("items", "nonexistent").is_none());
        assert!(registry.get("nonexistent", "potion").is_none());

        // all
        let all = registry.all("items").unwrap();
        assert_eq!(all.len(), 2);
        assert!(registry.all("nonexistent").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_mixed_files_and_dirs() {
        let dir = make_temp_dir("mixed");

        // Top-level array file
        fs::write(
            dir.join("items.json"),
            r#"[{"id": "sword", "name": "Sword"}]"#,
        )
        .unwrap();

        // Subdirectory with individual files
        let zones_dir = dir.join("zones");
        fs::create_dir_all(&zones_dir).unwrap();
        fs::write(
            zones_dir.join("town.json"),
            r#"{"id": "town", "name": "Town Square"}"#,
        )
        .unwrap();

        let registry = ContentRegistry::load_dir(&dir).unwrap();
        assert_eq!(registry.total_count(), 2);
        assert_eq!(registry.collection_names(), vec!["items", "zones"]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_not_a_directory() {
        let result = ContentRegistry::load_dir(Path::new("/tmp/nonexistent_content_dir_xyz"));
        assert!(result.is_err());
    }

    #[test]
    fn test_object_dir_missing_id() {
        let dir = make_temp_dir("objdir_no_id");
        let zones_dir = dir.join("zones");
        fs::create_dir_all(&zones_dir).unwrap();
        fs::write(
            zones_dir.join("broken.json"),
            r#"{"name": "No ID Zone"}"#,
        )
        .unwrap();

        let result = ContentRegistry::load_dir(&dir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("missing"), "error: {}", err);

        let _ = fs::remove_dir_all(&dir);
    }
}
