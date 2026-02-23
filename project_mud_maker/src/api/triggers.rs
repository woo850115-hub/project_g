use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_triggers).put(save_triggers))
        .route("/{id}", get(get_trigger).put(update_trigger).delete(delete_trigger))
        .route("/generate", post(generate_lua))
}

// --- Data model ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TriggerData {
    pub triggers: Vec<Trigger>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub condition: TriggerCondition,
    pub actions: Vec<TriggerAction>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TriggerCondition {
    #[serde(rename = "enter_room")]
    EnterRoom { room_id: String },
    #[serde(rename = "command")]
    Command { command: String },
    #[serde(rename = "tick_interval")]
    TickInterval { interval: u32 },
    #[serde(rename = "entity_death")]
    EntityDeath {
        #[serde(default)]
        content_id: String,
    },
    #[serde(rename = "on_connect")]
    OnConnect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TriggerAction {
    #[serde(rename = "send_message")]
    SendMessage {
        #[serde(default = "default_target")]
        target: String, // "player" or "room"
        text: String,
    },
    #[serde(rename = "spawn_entity")]
    SpawnEntity {
        entity_type: String, // "npc" or "item"
        content_id: String,
        room_id: String,
    },
    #[serde(rename = "teleport")]
    Teleport { room_id: String },
    #[serde(rename = "set_component")]
    SetComponent {
        #[serde(default = "default_target")]
        target: String, // "player" or "entity"
        component: String,
        value: Value,
    },
    #[serde(rename = "despawn_trigger_entity")]
    DespawnTriggerEntity,
    #[serde(rename = "give_item")]
    GiveItem { content_id: String },
    #[serde(rename = "heal")]
    Heal {
        #[serde(default = "default_target")]
        target: String, // "player" or "entity"
        mode: String,   // "full" | "percent" | "fixed"
        #[serde(default)]
        amount: u32, // percent: 1-100, fixed: HP amount. ignored for full
    },
}

fn default_target() -> String {
    "player".to_string()
}

// --- Handlers ---

async fn list_triggers(State(state): State<AppState>) -> impl IntoResponse {
    match load_triggers(&state) {
        Ok(data) => (StatusCode::OK, Json(serde_json::to_value(data.triggers).unwrap())),
        Err(e) => e,
    }
}

async fn save_triggers(
    State(state): State<AppState>,
    Json(triggers): Json<Vec<Trigger>>,
) -> impl IntoResponse {
    let data = TriggerData { triggers };
    match save_triggers_file(&state, &data) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => e,
    }
}

async fn get_trigger(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let data = match load_triggers(&state) {
        Ok(d) => d,
        Err(e) => return e,
    };

    match data.triggers.iter().find(|t| t.id == id) {
        Some(trigger) => (StatusCode::OK, Json(serde_json::to_value(trigger).unwrap())),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Trigger not found"})),
        ),
    }
}

async fn update_trigger(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut trigger): Json<Trigger>,
) -> impl IntoResponse {
    trigger.id = id.clone();
    let mut data = match load_triggers(&state) {
        Ok(d) => d,
        Err(e) => return e,
    };

    if let Some(existing) = data.triggers.iter_mut().find(|t| t.id == id) {
        *existing = trigger;
    } else {
        data.triggers.push(trigger);
    }

    match save_triggers_file(&state, &data) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => e,
    }
}

async fn delete_trigger(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut data = match load_triggers(&state) {
        Ok(d) => d,
        Err(e) => return e,
    };

    let len_before = data.triggers.len();
    data.triggers.retain(|t| t.id != id);

    if data.triggers.len() == len_before {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Trigger not found"})),
        );
    }

    match save_triggers_file(&state, &data) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => e,
    }
}

async fn generate_lua(State(state): State<AppState>) -> impl IntoResponse {
    let data = match load_triggers(&state) {
        Ok(d) => d,
        Err(e) => return e,
    };

    let lua = generate_triggers_lua(&data, &state);

    let scripts_dir = state.config.scripts_dir();
    if !scripts_dir.exists() {
        let _ = std::fs::create_dir_all(&scripts_dir);
    }

    let path = scripts_dir.join("05_triggers.lua");
    match std::fs::write(&path, &lua) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"ok": true, "path": "scripts/05_triggers.lua", "preview": lua})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write: {e}")})),
        ),
    }
}

// --- File I/O ---

fn triggers_file_path(state: &AppState) -> std::path::PathBuf {
    std::path::PathBuf::from(&state.config.project.mud_dir).join("triggers.json")
}

fn load_triggers(state: &AppState) -> Result<TriggerData, (StatusCode, Json<Value>)> {
    let path = triggers_file_path(state);
    if !path.exists() {
        return Ok(TriggerData::default());
    }

    let text = std::fs::read_to_string(&path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to read triggers.json: {e}")})),
        )
    })?;

    serde_json::from_str(&text).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Invalid triggers.json: {e}")})),
        )
    })
}

fn save_triggers_file(
    state: &AppState,
    data: &TriggerData,
) -> Result<(), (StatusCode, Json<Value>)> {
    let path = triggers_file_path(state);
    let json = serde_json::to_string_pretty(data).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Serialize error: {e}")})),
        )
    })?;

    std::fs::write(&path, json).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write triggers.json: {e}")})),
        )
    })
}

// --- Lua Generator ---

fn generate_triggers_lua(data: &TriggerData, state: &AppState) -> String {
    let mut lua = String::new();
    lua.push_str("-- 05_triggers.lua (auto-generated by MUD Game Maker)\n");
    lua.push_str("-- DO NOT EDIT MANUALLY — changes will be overwritten\n\n");

    let enabled: Vec<&Trigger> = data.triggers.iter().filter(|t| t.enabled).collect();
    if enabled.is_empty() {
        lua.push_str("-- No triggers defined\n");
        return lua;
    }

    // Group triggers by condition type for efficient code generation
    let mut enter_room_triggers: Vec<&Trigger> = vec![];
    let mut command_triggers: Vec<&Trigger> = vec![];
    let mut tick_triggers: Vec<&Trigger> = vec![];
    let mut death_triggers: Vec<&Trigger> = vec![];
    let mut connect_triggers: Vec<&Trigger> = vec![];

    for t in &enabled {
        match &t.condition {
            TriggerCondition::EnterRoom { .. } => enter_room_triggers.push(t),
            TriggerCondition::Command { .. } => command_triggers.push(t),
            TriggerCondition::TickInterval { .. } => tick_triggers.push(t),
            TriggerCondition::EntityDeath { .. } => death_triggers.push(t),
            TriggerCondition::OnConnect => connect_triggers.push(t),
        }
    }

    let content_dir = state.config.content_dir();

    // on_enter_room triggers
    if !enter_room_triggers.is_empty() {
        lua.push_str("-- Room entry triggers\n");
        lua.push_str("hooks.on_enter_room(function(entity, room, old_room)\n");
        lua.push_str("    local room_name = ecs:get(room, \"Name\") or \"\"\n");
        for t in &enter_room_triggers {
            if let TriggerCondition::EnterRoom { room_id } = &t.condition {
                lua.push_str(&format!(
                    "    if room_name == \"{}\" then -- {}\n",
                    escape_lua(room_id),
                    escape_lua(&t.name)
                ));
                gen_actions(&mut lua, &t.actions, "        ", &content_dir);
                lua.push_str("    end\n");
            }
        }
        lua.push_str("end)\n\n");
    }

    // command triggers
    for t in &command_triggers {
        if let TriggerCondition::Command { command } = &t.condition {
            lua.push_str(&format!("-- Trigger: {}\n", escape_lua(&t.name)));
            lua.push_str(&format!(
                "hooks.on_action(\"{}\", function(ctx)\n",
                escape_lua(command)
            ));
            gen_actions(&mut lua, &t.actions, "    ", &content_dir);
            lua.push_str("    return true\n");
            lua.push_str("end)\n\n");
        }
    }

    // tick triggers
    if !tick_triggers.is_empty() || !death_triggers.is_empty() {
        lua.push_str("-- Tick-based triggers\n");
        lua.push_str("hooks.on_tick(function(tick)\n");

        for t in &tick_triggers {
            if let TriggerCondition::TickInterval { interval } = &t.condition {
                lua.push_str(&format!(
                    "    if tick % {} == 0 then -- {}\n",
                    interval,
                    escape_lua(&t.name)
                ));
                gen_actions(&mut lua, &t.actions, "        ", &content_dir);
                lua.push_str("    end\n");
            }
        }

        // Death detection triggers
        for t in &death_triggers {
            if let TriggerCondition::EntityDeath { content_id } = &t.condition {
                lua.push_str(&format!("    -- Death trigger: {}\n", escape_lua(&t.name)));
                lua.push_str("    local dead_entities = ecs:query(\"Dead\")\n");
                lua.push_str("    for _, dead_ent in ipairs(dead_entities) do\n");
                if !content_id.is_empty() {
                    lua.push_str(&format!(
                        "        local name = ecs:get(dead_ent, \"Name\") or \"\"\n"
                    ));
                    lua.push_str(&format!(
                        "        if name == \"{}\" then\n",
                        escape_lua(content_id)
                    ));
                    gen_actions(&mut lua, &t.actions, "            ", &content_dir);
                    lua.push_str("        end\n");
                } else {
                    gen_actions(&mut lua, &t.actions, "        ", &content_dir);
                }
                lua.push_str("    end\n");
            }
        }

        lua.push_str("end)\n\n");
    }

    // connect triggers
    if !connect_triggers.is_empty() {
        lua.push_str("-- Connection triggers\n");
        lua.push_str("hooks.on_connect(function(session_id)\n");
        for t in &connect_triggers {
            lua.push_str(&format!("    -- {}\n", escape_lua(&t.name)));
            gen_actions(&mut lua, &t.actions, "    ", &content_dir);
        }
        lua.push_str("end)\n\n");
    }

    lua
}

fn gen_actions(lua: &mut String, actions: &[TriggerAction], indent: &str, content_dir: &std::path::Path) {
    for action in actions {
        match action {
            TriggerAction::SendMessage { target, text } => {
                let escaped = escape_lua(text);
                match target.as_str() {
                    "room" => {
                        lua.push_str(&format!(
                            "{indent}output:broadcast_room(room, \"{}\")\n",
                            escaped
                        ));
                    }
                    _ => {
                        lua.push_str(&format!(
                            "{indent}local sid = sessions:session_for(entity)\n"
                        ));
                        lua.push_str(&format!(
                            "{indent}if sid then output:send(sid, \"{}\") end\n",
                            escaped
                        ));
                    }
                }
            }
            TriggerAction::SpawnEntity {
                entity_type,
                content_id,
                room_id,
            } => {
                let var = format!("spawned_{}", sanitize_var(content_id));
                lua.push_str(&format!("{indent}local {var} = ecs:spawn()\n"));

                // Load content for stats
                let content = load_content_item(content_dir, entity_type, content_id);
                let name = content
                    .as_ref()
                    .and_then(|c| c.get("name").and_then(|v| v.as_str()))
                    .unwrap_or(content_id);
                lua.push_str(&format!(
                    "{indent}ecs:set({var}, \"Name\", \"{}\")\n",
                    escape_lua(name)
                ));

                match entity_type.as_str() {
                    "npc" => {
                        lua.push_str(&format!(
                            "{indent}ecs:set({var}, \"NpcTag\", true)\n"
                        ));
                        if let Some(c) = &content {
                            if let Some(hp) = c.get("hp").and_then(|v| v.as_i64()) {
                                lua.push_str(&format!(
                                    "{indent}ecs:set({var}, \"Health\", {{current = {hp}, max = {hp}}})\n"
                                ));
                            }
                            if let Some(atk) = c.get("attack").and_then(|v| v.as_i64()) {
                                lua.push_str(&format!(
                                    "{indent}ecs:set({var}, \"Attack\", {atk})\n"
                                ));
                            }
                            if let Some(def) = c.get("defense").and_then(|v| v.as_i64()) {
                                lua.push_str(&format!(
                                    "{indent}ecs:set({var}, \"Defense\", {def})\n"
                                ));
                            }
                        }
                    }
                    "item" => {
                        lua.push_str(&format!(
                            "{indent}ecs:set({var}, \"ItemTag\", true)\n"
                        ));
                    }
                    _ => {}
                }

                // Place in room by name
                lua.push_str(&format!(
                    "{indent}local target_room = nil\n"
                ));
                lua.push_str(&format!(
                    "{indent}for _, r in ipairs(space:all_rooms()) do\n"
                ));
                lua.push_str(&format!(
                    "{indent}    if ecs:get(r, \"Name\") == \"{}\" then target_room = r break end\n",
                    escape_lua(room_id)
                ));
                lua.push_str(&format!("{indent}end\n"));
                lua.push_str(&format!(
                    "{indent}if target_room then space:place_entity({var}, target_room) end\n"
                ));
            }
            TriggerAction::Teleport { room_id } => {
                lua.push_str(&format!(
                    "{indent}local dest_room = nil\n"
                ));
                lua.push_str(&format!(
                    "{indent}for _, r in ipairs(space:all_rooms()) do\n"
                ));
                lua.push_str(&format!(
                    "{indent}    if ecs:get(r, \"Name\") == \"{}\" then dest_room = r break end\n",
                    escape_lua(room_id)
                ));
                lua.push_str(&format!("{indent}end\n"));
                lua.push_str(&format!(
                    "{indent}if dest_room then space:place_entity(entity, dest_room) end\n"
                ));
            }
            TriggerAction::SetComponent {
                component, value, ..
            } => {
                let val_str = json_to_lua_value(value);
                lua.push_str(&format!(
                    "{indent}ecs:set(entity, \"{}\", {val_str})\n",
                    escape_lua(component)
                ));
            }
            TriggerAction::DespawnTriggerEntity => {
                lua.push_str(&format!("{indent}ecs:despawn(entity)\n"));
            }
            TriggerAction::GiveItem { content_id } => {
                let var = format!("item_{}", sanitize_var(content_id));
                lua.push_str(&format!("{indent}local {var} = ecs:spawn()\n"));

                let content = load_content_item(content_dir, "item", content_id);
                let name = content
                    .as_ref()
                    .and_then(|c| c.get("name").and_then(|v| v.as_str()))
                    .unwrap_or(content_id);
                lua.push_str(&format!(
                    "{indent}ecs:set({var}, \"Name\", \"{}\")\n",
                    escape_lua(name)
                ));
                lua.push_str(&format!(
                    "{indent}ecs:set({var}, \"ItemTag\", true)\n"
                ));
                lua.push_str(&format!(
                    "{indent}ecs:set({var}, \"Inventory\", entity)\n"
                ));
            }
            TriggerAction::Heal { mode, amount, .. } => {
                lua.push_str(&format!("{indent}do\n"));
                lua.push_str(&format!(
                    "{indent}    local hp = ecs:get(entity, \"Health\")\n"
                ));
                lua.push_str(&format!("{indent}    if hp then\n"));
                match mode.as_str() {
                    "percent" => {
                        lua.push_str(&format!(
                            "{indent}        hp.current = math.min(hp.current + math.floor(hp.max * {} / 100), hp.max)\n",
                            amount
                        ));
                    }
                    "fixed" => {
                        lua.push_str(&format!(
                            "{indent}        hp.current = math.min(hp.current + {}, hp.max)\n",
                            amount
                        ));
                    }
                    _ => {
                        // "full" or default
                        lua.push_str(&format!(
                            "{indent}        hp.current = hp.max\n"
                        ));
                    }
                }
                lua.push_str(&format!(
                    "{indent}        ecs:set(entity, \"Health\", hp)\n"
                ));
                lua.push_str(&format!("{indent}    end\n"));
                lua.push_str(&format!("{indent}end\n"));
            }
        }
    }
}

fn load_content_item(
    content_dir: &std::path::Path,
    entity_type: &str,
    content_id: &str,
) -> Option<Value> {
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
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if s.is_empty() || s.chars().next().map_or(true, |c| c.is_ascii_digit()) {
        format!("v_{s}")
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

/// Convert a serde_json::Value to a valid Lua literal.
fn json_to_lua_value(value: &Value) -> String {
    match value {
        Value::Null => "nil".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("\"{}\"", escape_lua(s)),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(json_to_lua_value).collect();
            format!("{{{}}}", items.join(", "))
        }
        Value::Object(map) => {
            let pairs: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{} = {}", k, json_to_lua_value(v)))
                .collect();
            format!("{{{}}}", pairs.join(", "))
        }
    }
}
