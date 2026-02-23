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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorldData {
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

    // Create room entities
    lua.push_str("    -- Rooms\n");
    for room in &world.rooms {
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

            // Set tag
            match placed.entity_type.as_str() {
                "npc" => {
                    lua.push_str(&format!("    ecs:set({ent_var}, \"NpcTag\", true)\n"));
                    // Set combat stats from content
                    if let Some(c) = &content {
                        if let Some(hp) = c.get("hp").and_then(|v| v.as_i64()) {
                            lua.push_str(&format!(
                                "    ecs:set({ent_var}, \"Health\", {{current = {hp}, max = {hp}}})\n"
                            ));
                        }
                        if let Some(atk) = c.get("attack").and_then(|v| v.as_i64()) {
                            lua.push_str(&format!(
                                "    ecs:set({ent_var}, \"Attack\", {atk})\n"
                            ));
                        }
                        if let Some(def) = c.get("defense").and_then(|v| v.as_i64()) {
                            lua.push_str(&format!(
                                "    ecs:set({ent_var}, \"Defense\", {def})\n"
                            ));
                        }
                    }
                }
                "item" => {
                    lua.push_str(&format!("    ecs:set({ent_var}, \"ItemTag\", true)\n"));
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
