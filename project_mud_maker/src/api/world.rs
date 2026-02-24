use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_world).put(save_world))
        .route("/rooms/{id}", get(get_room).put(update_room).delete(delete_room))
        .route("/rooms/{id}/entities", get(get_room_entities).put(update_room_entities))
        .route("/generate", post(generate_lua))
}

// --- Data model ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
    #[serde(default = "default_zone_color")]
    pub color: String,
}

fn default_zone_color() -> String {
    "#3b82f6".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorldData {
    #[serde(default)]
    pub zones: Vec<Zone>,
    pub rooms: Vec<Room>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub position: Position,
    #[serde(default)]
    pub exits: BTreeMap<String, String>,
    #[serde(default)]
    pub entities: Vec<PlacedEntity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacedEntity {
    #[serde(rename = "type")]
    pub entity_type: String, // "npc" or "item"
    pub content_id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<String, Value>,
}

// --- Handlers ---

/// GET /api/world — get full world data
async fn get_world(State(state): State<AppState>) -> impl IntoResponse {
    match load_world(&state) {
        Ok(world) => (StatusCode::OK, Json(serde_json::to_value(world).unwrap())),
        Err(e) => e,
    }
}

/// PUT /api/world — save full world data
async fn save_world(
    State(state): State<AppState>,
    Json(world): Json<WorldData>,
) -> impl IntoResponse {
    match save_world_file(&state, &world) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => e,
    }
}

/// GET /api/world/rooms/:id
async fn get_room(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let world = match load_world(&state) {
        Ok(w) => w,
        Err(e) => return e,
    };

    match world.rooms.iter().find(|r| r.id == id) {
        Some(room) => (StatusCode::OK, Json(serde_json::to_value(room).unwrap())),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Room not found"})),
        ),
    }
}

/// PUT /api/world/rooms/:id — create or update a room
async fn update_room(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut room): Json<Room>,
) -> impl IntoResponse {
    room.id = id.clone();
    let mut world = match load_world(&state) {
        Ok(w) => w,
        Err(e) => return e,
    };

    if let Some(existing) = world.rooms.iter_mut().find(|r| r.id == id) {
        *existing = room;
    } else {
        world.rooms.push(room);
    }

    match save_world_file(&state, &world) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => e,
    }
}

/// DELETE /api/world/rooms/:id
async fn delete_room(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut world = match load_world(&state) {
        Ok(w) => w,
        Err(e) => return e,
    };

    let len_before = world.rooms.len();
    world.rooms.retain(|r| r.id != id);

    if world.rooms.len() == len_before {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Room not found"})),
        );
    }

    // Clean up exits pointing to deleted room
    for room in world.rooms.iter_mut() {
        room.exits.retain(|_, target| target != &id);
    }

    match save_world_file(&state, &world) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => e,
    }
}

/// GET /api/world/rooms/:id/entities
async fn get_room_entities(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let world = match load_world(&state) {
        Ok(w) => w,
        Err(e) => return e,
    };

    match world.rooms.iter().find(|r| r.id == id) {
        Some(room) => (
            StatusCode::OK,
            Json(serde_json::to_value(&room.entities).unwrap()),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Room not found"})),
        ),
    }
}

/// PUT /api/world/rooms/:id/entities
async fn update_room_entities(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(entities): Json<Vec<PlacedEntity>>,
) -> impl IntoResponse {
    let mut world = match load_world(&state) {
        Ok(w) => w,
        Err(e) => return e,
    };

    match world.rooms.iter_mut().find(|r| r.id == id) {
        Some(room) => {
            room.entities = entities;
        }
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Room not found"})),
            );
        }
    }

    match save_world_file(&state, &world) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => e,
    }
}

/// POST /api/world/generate — generate Lua from world.json
async fn generate_lua(State(state): State<AppState>) -> impl IntoResponse {
    let world = match load_world(&state) {
        Ok(w) => w,
        Err(e) => return e,
    };

    let lua = generate_world_lua(&world, &state);

    let scripts_dir = state.config.scripts_dir();
    if !scripts_dir.exists() {
        let _ = std::fs::create_dir_all(&scripts_dir);
    }

    let path = scripts_dir.join("01_world_setup.lua");
    match std::fs::write(&path, &lua) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"ok": true, "path": "scripts/01_world_setup.lua", "preview": lua})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write Lua: {e}")})),
        ),
    }
}

/// Called by generate_all to generate world Lua without going through axum handler.
pub fn generate_world_lua_inner(state: &AppState) -> Result<String, String> {
    let world = load_world_sync(state)?;
    let lua = generate_world_lua(&world, state);
    let scripts_dir = state.config.scripts_dir();
    if !scripts_dir.exists() {
        let _ = std::fs::create_dir_all(&scripts_dir);
    }
    let path = scripts_dir.join("01_world_setup.lua");
    std::fs::write(&path, &lua).map_err(|e| format!("Failed to write: {e}"))?;
    Ok("scripts/01_world_setup.lua".to_string())
}

fn load_world_sync(state: &AppState) -> Result<WorldData, String> {
    let path = world_file_path(state);
    if !path.exists() {
        return Ok(WorldData::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Read error: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))
}

// --- File I/O ---

fn world_file_path(state: &AppState) -> std::path::PathBuf {
    std::path::PathBuf::from(&state.config.project.mud_dir).join("world.json")
}

fn load_world(state: &AppState) -> Result<WorldData, (StatusCode, Json<Value>)> {
    let path = world_file_path(state);
    if !path.exists() {
        return Ok(WorldData::default());
    }

    let text = std::fs::read_to_string(&path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to read world.json: {e}")})),
        )
    })?;

    serde_json::from_str(&text).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Invalid world.json: {e}")})),
        )
    })
}

fn save_world_file(state: &AppState, world: &WorldData) -> Result<(), (StatusCode, Json<Value>)> {
    let path = world_file_path(state);
    let json = serde_json::to_string_pretty(world).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Serialize error: {e}")})),
        )
    })?;

    std::fs::write(&path, json).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write world.json: {e}")})),
        )
    })
}

// --- Lua Generator ---

fn generate_world_lua(world: &WorldData, state: &AppState) -> String {
    let mut lua = String::new();
    lua.push_str("-- 01_world_setup.lua (auto-generated by MUD Game Maker)\n");
    lua.push_str("-- DO NOT EDIT MANUALLY — changes will be overwritten\n\n");

    // Generate CONTENT_DATA global table before on_init
    generate_content_data(&mut lua, state);

    lua.push_str("hooks.on_init(function()\n");
    lua.push_str("    if space:room_count() > 0 then\n");
    lua.push_str("        log.info(\"World already loaded from snapshot, skipping creation\")\n");
    lua.push_str("        return\n");
    lua.push_str("    end\n\n");
    lua.push_str("    log.info(\"Creating world...\")\n\n");

    if world.rooms.is_empty() {
        lua.push_str("    log.warn(\"No rooms defined in world.json\")\n");
        lua.push_str("end)\n");
        return lua;
    }

    // Group rooms by zone for organized output
    let zone_map: BTreeMap<&str, &Zone> = world.zones.iter().map(|z| (z.id.as_str(), z)).collect();
    let mut rooms_by_zone: BTreeMap<Option<&str>, Vec<&Room>> = BTreeMap::new();
    for room in &world.rooms {
        let key = room.zone_id.as_deref();
        rooms_by_zone.entry(key).or_default().push(room);
    }

    // Create room entities (grouped by zone)
    for (zone_id, zone_rooms) in &rooms_by_zone {
        match zone_id {
            Some(zid) => {
                let zone_name = zone_map.get(zid).map(|z| z.name.as_str()).unwrap_or(zid);
                lua.push_str(&format!("    -- === Zone: {} ===\n", zone_name));
            }
            None => {
                lua.push_str("    -- === Zone: unassigned ===\n");
            }
        }
        for room in zone_rooms {
            let var = sanitize_var(&room.id);
            lua.push_str(&format!("    local {var} = ecs:spawn()\n"));
            lua.push_str(&format!(
                "    ecs:set({var}, \"Name\", \"{}\")\n",
                escape_lua(&room.name)
            ));
            if !room.description.is_empty() {
                lua.push_str(&format!(
                    "    ecs:set({var}, \"Description\", \"{}\")\n",
                    escape_lua(&room.description)
                ));
            }
        }
        lua.push('\n');
    }

    // Register rooms with exits
    lua.push_str("    -- Exits\n");
    for room in &world.rooms {
        let var = sanitize_var(&room.id);
        if room.exits.is_empty() {
            lua.push_str(&format!("    space:register_room({var}, {{}})\n"));
        } else {
            let exits: Vec<String> = room
                .exits
                .iter()
                .map(|(dir, target)| format!("{dir} = {}", sanitize_var(target)))
                .collect();
            lua.push_str(&format!(
                "    space:register_room({var}, {{{}}})\n",
                exits.join(", ")
            ));
        }
    }
    lua.push('\n');

    // Load attribute schemas for GameData generation
    let schemas = super::attribute_schemas::load_schemas_sync(state);
    // Collect schema field IDs that apply to each entity type
    let npc_schema_ids: Vec<&str> = schemas
        .iter()
        .filter(|s| s.applies_to.is_empty() || s.applies_to.iter().any(|a| a == "monsters"))
        .map(|s| s.id.as_str())
        .collect();
    let item_schema_ids: Vec<&str> = schemas
        .iter()
        .filter(|s| s.applies_to.is_empty() || s.applies_to.iter().any(|a| a == "items"))
        .map(|s| s.id.as_str())
        .collect();

    // ECS component keys (not GameData)
    let ecs_keys: &[&str] = &[
        "id", "name", "description", "hp", "attack", "defense",
        "gold", "level", "race", "class", "skills",
    ];

    // Place entities in rooms
    let mut entity_count = 0;
    let content_dir = state.config.content_dir();

    for room in &world.rooms {
        if room.entities.is_empty() {
            continue;
        }
        let room_var = sanitize_var(&room.id);
        lua.push_str(&format!("    -- Entities in {}\n", room.name));

        for (i, placed) in room.entities.iter().enumerate() {
            entity_count += 1;
            let ent_var = format!("{room_var}_ent_{i}");
            lua.push_str(&format!("    local {ent_var} = ecs:spawn()\n"));

            // Try to load content definition
            let content = load_content_item(&content_dir, &placed.entity_type, &placed.content_id);

            // Set Name from content or override
            let name = placed
                .overrides
                .get("name")
                .and_then(|v| v.as_str())
                .or_else(|| content.as_ref().and_then(|c| c.get("name").and_then(|v| v.as_str())))
                .unwrap_or(&placed.content_id);
            lua.push_str(&format!(
                "    ecs:set({ent_var}, \"Name\", \"{}\")\n",
                escape_lua(name)
            ));

            // Set Description
            if let Some(desc) = placed
                .overrides
                .get("description")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    content
                        .as_ref()
                        .and_then(|c| c.get("description").and_then(|v| v.as_str()))
                })
            {
                lua.push_str(&format!(
                    "    ecs:set({ent_var}, \"Description\", \"{}\")\n",
                    escape_lua(desc)
                ));
            }

            // Set tag + all applicable ECS components
            match placed.entity_type.as_str() {
                "npc" => {
                    lua.push_str(&format!("    ecs:set({ent_var}, \"NpcTag\", true)\n"));
                    if let Some(c) = &content {
                        // Health (from hp field)
                        if let Some(hp) =
                            get_override_or_content_i64(&placed.overrides, c, "hp")
                        {
                            lua.push_str(&format!(
                                "    ecs:set({ent_var}, \"Health\", {{current = {hp}, max = {hp}}})\n"
                            ));
                        }
                        // Attack
                        if let Some(atk) =
                            get_override_or_content_i64(&placed.overrides, c, "attack")
                        {
                            lua.push_str(&format!(
                                "    ecs:set({ent_var}, \"Attack\", {atk})\n"
                            ));
                        }
                        // Defense
                        if let Some(def) =
                            get_override_or_content_i64(&placed.overrides, c, "defense")
                        {
                            lua.push_str(&format!(
                                "    ecs:set({ent_var}, \"Defense\", {def})\n"
                            ));
                        }
                        // Gold
                        if let Some(gold) =
                            get_override_or_content_i64(&placed.overrides, c, "gold")
                        {
                            lua.push_str(&format!(
                                "    ecs:set({ent_var}, \"Gold\", {gold})\n"
                            ));
                        }
                        // Level
                        if let Some(level) =
                            get_override_or_content_i64(&placed.overrides, c, "level")
                        {
                            let exp_next = level * 100;
                            lua.push_str(&format!(
                                "    ecs:set({ent_var}, \"Level\", {{level = {level}, exp = 0, exp_next = {exp_next}}})\n"
                            ));
                        }
                        // Race
                        if let Some(race) =
                            get_override_or_content_str(&placed.overrides, c, "race")
                        {
                            lua.push_str(&format!(
                                "    ecs:set({ent_var}, \"Race\", \"{}\")\n",
                                escape_lua(&race)
                            ));
                        }
                        // Class
                        if let Some(class) =
                            get_override_or_content_str(&placed.overrides, c, "class")
                        {
                            lua.push_str(&format!(
                                "    ecs:set({ent_var}, \"Class\", \"{}\")\n",
                                escape_lua(&class)
                            ));
                        }
                        // Skills
                        let skills = placed
                            .overrides
                            .get("skills")
                            .and_then(|v| v.as_array())
                            .or_else(|| c.get("skills").and_then(|v| v.as_array()));
                        if let Some(skills) = skills {
                            let skill_list: Vec<String> = skills
                                .iter()
                                .filter_map(|s| {
                                    s.as_str()
                                        .map(|s| format!("\"{}\"", escape_lua(s)))
                                })
                                .collect();
                            if !skill_list.is_empty() {
                                lua.push_str(&format!(
                                    "    ecs:set({ent_var}, \"Skills\", {{learned = {{{}}}}})\n",
                                    skill_list.join(", ")
                                ));
                            }
                        }

                        // GameData — collect schema-defined custom attributes
                        let game_data = collect_game_data(
                            &placed.overrides,
                            c,
                            &npc_schema_ids,
                            ecs_keys,
                        );
                        if !game_data.is_empty() {
                            lua.push_str(&format!(
                                "    ecs:set({ent_var}, \"GameData\", {})\n",
                                json_value_to_lua(&Value::Object(game_data))
                            ));
                        }
                    }
                }
                "item" => {
                    lua.push_str(&format!("    ecs:set({ent_var}, \"ItemTag\", true)\n"));

                    // GameData for items
                    if let Some(c) = &content {
                        let game_data = collect_game_data(
                            &placed.overrides,
                            c,
                            &item_schema_ids,
                            ecs_keys,
                        );
                        if !game_data.is_empty() {
                            lua.push_str(&format!(
                                "    ecs:set({ent_var}, \"GameData\", {})\n",
                                json_value_to_lua(&Value::Object(game_data))
                            ));
                        }
                    }
                }
                _ => {}
            }

            lua.push_str(&format!("    space:place_entity({ent_var}, {room_var})\n"));
        }
        lua.push('\n');
    }

    lua.push_str(&format!(
        "    log.info(\"World created: {} rooms, {} entities\")\n",
        world.rooms.len(),
        entity_count
    ));
    lua.push_str("end)\n");

    lua
}

fn load_content_item(
    content_dir: &std::path::Path,
    entity_type: &str,
    content_id: &str,
) -> Option<Value> {
    // Try collection name based on type: "npc" -> "monsters.json", "item" -> "items.json"
    let collection = match entity_type {
        "npc" => "monsters",
        "item" => "items",
        other => other,
    };

    let path = content_dir.join(format!("{collection}.json"));
    let text = std::fs::read_to_string(path).ok()?;
    let items: Vec<Value> = serde_json::from_str(&text).ok()?;
    items
        .into_iter()
        .find(|item| item.get("id").and_then(|v| v.as_str()) == Some(content_id))
}

fn sanitize_var(id: &str) -> String {
    let s: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if s.is_empty() || s.chars().next().unwrap().is_ascii_digit() {
        format!("room_{s}")
    } else {
        s
    }
}

fn escape_lua(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "")
}

// --- Attribute helpers ---

/// Get i64 value: override takes priority, then content JSON
fn get_override_or_content_i64(
    overrides: &BTreeMap<String, Value>,
    content: &Value,
    key: &str,
) -> Option<i64> {
    overrides
        .get(key)
        .and_then(|v| v.as_i64())
        .or_else(|| content.get(key).and_then(|v| v.as_i64()))
}

/// Get string value: override takes priority, then content JSON
fn get_override_or_content_str(
    overrides: &BTreeMap<String, Value>,
    content: &Value,
    key: &str,
) -> Option<String> {
    overrides
        .get(key)
        .and_then(|v| v.as_str())
        .or_else(|| content.get(key).and_then(|v| v.as_str()))
        .map(|s| s.to_string())
}

/// Convert a serde_json::Value to Lua literal syntax
fn json_value_to_lua(value: &Value) -> String {
    match value {
        Value::Null => "nil".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("\"{}\"", escape_lua(s)),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(json_value_to_lua).collect();
            format!("{{{}}}", items.join(", "))
        }
        Value::Object(obj) => {
            let entries: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{} = {}", sanitize_var(k), json_value_to_lua(v)))
                .collect();
            format!("{{{}}}", entries.join(", "))
        }
    }
}

/// Generate CONTENT_DATA global table for script reference
fn generate_content_data(lua: &mut String, state: &AppState) {
    let content_dir = state.config.content_dir();
    if !content_dir.exists() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(&content_dir) else {
        return;
    };
    let mut collections: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
        .collect();
    collections.sort_by_key(|e| e.file_name());

    let mut data_parts = Vec::new();

    for entry in collections {
        let path = entry.path();
        let collection = path.file_stem().unwrap().to_string_lossy().to_string();
        // Skip non-content files (world, shops, dialogues, quests, attribute_schema are handled separately)
        if matches!(
            collection.as_str(),
            "world" | "shops" | "dialogues" | "quests" | "attribute_schema"
        ) {
            continue;
        }

        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(items): Result<Vec<Value>, _> = serde_json::from_str(&text) else {
            continue;
        };
        if items.is_empty() {
            continue;
        }

        let mut col_lua = format!("CONTENT_DATA[\"{}\"] = {{\n", escape_lua(&collection));
        for item in &items {
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .or_else(|| item.get("id").and_then(|v| v.as_str()))
                .unwrap_or("unknown");
            col_lua.push_str(&format!(
                "    [\"{}\"] = {},\n",
                escape_lua(name),
                json_value_to_lua(item)
            ));
        }
        col_lua.push_str("}\n");
        data_parts.push(col_lua);
    }

    if !data_parts.is_empty() {
        lua.push_str("-- Content metadata (accessible from any script via CONTENT_DATA)\n");
        lua.push_str("CONTENT_DATA = CONTENT_DATA or {}\n");
        for part in &data_parts {
            lua.push_str(part);
        }
        lua.push('\n');
    }
}

/// Collect custom attribute values from overrides and content for GameData.
/// Returns only fields that match schema_ids and are NOT in ecs_keys.
fn collect_game_data(
    overrides: &BTreeMap<String, Value>,
    content: &Value,
    schema_ids: &[&str],
    ecs_keys: &[&str],
) -> serde_json::Map<String, Value> {
    let mut data = serde_json::Map::new();
    for &key in schema_ids {
        if ecs_keys.contains(&key) {
            continue;
        }
        // Override takes priority
        if let Some(val) = overrides.get(key) {
            if !val.is_null() {
                data.insert(key.to_string(), val.clone());
                continue;
            }
        }
        // Then content
        if let Some(val) = content.get(key) {
            if !val.is_null() {
                data.insert(key.to_string(), val.clone());
            }
        }
    }
    data
}
