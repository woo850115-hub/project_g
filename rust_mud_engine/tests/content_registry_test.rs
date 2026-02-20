/// Integration tests for ContentRegistry + Lua scripting engine.
/// Verifies that JSON content files are loaded and accessible from Lua scripts.
use scripting::engine::{ScriptContext, ScriptEngine};
use scripting::{ContentRegistry, ScriptConfig};
use ecs_adapter::EcsAdapter;
use session::SessionManager;
use space::RoomGraphSpace;
use space::room_graph::RoomExits;
use std::fs;
use std::path::PathBuf;

fn make_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("content_integ_test_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn setup_world() -> (EcsAdapter, RoomGraphSpace, SessionManager) {
    let ecs = EcsAdapter::new();
    let mut space = RoomGraphSpace::new();
    let room = ecs_adapter::EntityId::new(100, 0);
    space.register_room(room, RoomExits::default());
    let sessions = SessionManager::new();
    (ecs, space, sessions)
}

/// Test: Load content directory -> ScriptEngine -> Lua access
#[test]
fn test_content_loaded_into_lua() {
    let dir = make_temp_dir("loaded_into_lua");
    let json = r#"[
        {"id": "goblin", "name": "Goblin", "hp_max": 30, "attack": 5},
        {"id": "orc", "name": "Orc", "hp_max": 80, "attack": 12}
    ]"#;
    fs::write(dir.join("monsters.json"), json).unwrap();

    let items_json = r#"[
        {"id": "potion", "name": "Health Potion", "heal": 50}
    ]"#;
    fs::write(dir.join("items.json"), items_json).unwrap();

    let registry = ContentRegistry::load_dir(&dir).unwrap();
    assert_eq!(registry.total_count(), 3);

    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine.register_content(&registry).unwrap();

    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                -- Access monster
                local goblin = content.monsters.goblin
                output:send(1, goblin.name .. ":" .. tostring(goblin.hp_max))

                -- Access item
                local potion = content.items.potion
                output:send(1, potion.name .. ":" .. tostring(potion.heal))

                -- Iterate monsters
                local count = 0
                for id, mon in pairs(content.monsters) do
                    count = count + 1
                end
                output:send(1, "monster_count:" .. tostring(count))

                -- Nil check for non-existent
                if content.monsters.dragon == nil then
                    output:send(1, "no_dragon")
                end
            end)
        "#,
        )
        .unwrap();

    let (mut ecs, mut space, sessions) = setup_world();
    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 4);
    assert_eq!(outputs[0].text, "Goblin:30");
    assert_eq!(outputs[1].text, "Health Potion:50");
    assert_eq!(outputs[2].text, "monster_count:2");
    assert_eq!(outputs[3].text, "no_dragon");

    let _ = fs::remove_dir_all(&dir);
}

/// Test: Subdirectory loading (zones/)
#[test]
fn test_content_subdirectory_loading() {
    let dir = make_temp_dir("subdir_loading");
    let zones_dir = dir.join("zones");
    fs::create_dir_all(&zones_dir).unwrap();

    fs::write(
        zones_dir.join("forest.json"),
        r#"{"id": "forest", "name": "Dark Forest", "level_range": [5, 10]}"#,
    )
    .unwrap();
    fs::write(
        zones_dir.join("cave.json"),
        r#"{"id": "cave", "name": "Crystal Cave", "level_range": [10, 15]}"#,
    )
    .unwrap();

    let registry = ContentRegistry::load_dir(&dir).unwrap();
    assert_eq!(registry.total_count(), 2);

    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine.register_content(&registry).unwrap();

    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                local forest = content.zones.forest
                output:send(1, forest.name)

                local cave = content.zones.cave
                output:send(1, cave.name)

                -- Access nested array
                output:send(1, tostring(forest.level_range[1]) .. "-" .. tostring(forest.level_range[2]))
            end)
        "#,
        )
        .unwrap();

    let (mut ecs, mut space, sessions) = setup_world();
    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 3);
    assert_eq!(outputs[0].text, "Dark Forest");
    assert_eq!(outputs[1].text, "Crystal Cave");
    assert_eq!(outputs[2].text, "5-10");

    let _ = fs::remove_dir_all(&dir);
}

/// Test: Content available in on_tick hook (not just on_init)
#[test]
fn test_content_in_hooks() {
    let dir = make_temp_dir("in_hooks");
    fs::write(
        dir.join("skills.json"),
        r#"[{"id": "fireball", "name": "Fireball", "damage": 100}]"#,
    )
    .unwrap();

    let registry = ContentRegistry::load_dir(&dir).unwrap();
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine.register_content(&registry).unwrap();

    engine
        .load_script(
            "test",
            r#"
            hooks.on_tick(function(tick)
                local fb = content.skills.fireball
                if fb then
                    output:send(1, "tick" .. tostring(tick) .. ":" .. fb.name .. "=" .. tostring(fb.damage))
                end
            end)
        "#,
        )
        .unwrap();

    let (mut ecs, mut space, sessions) = setup_world();
    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 5,
    };

    let outputs = engine.run_on_tick(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].text, "tick5:Fireball=100");

    let _ = fs::remove_dir_all(&dir);
}

/// Test: No content directory -> server still starts, content table is not set
#[test]
fn test_no_content_dir_graceful() {
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();

    // Don't register any content — simulates no content/ directory
    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                if content == nil then
                    output:send(1, "no_content")
                else
                    output:send(1, "has_content")
                end
            end)
        "#,
        )
        .unwrap();

    let (mut ecs, mut space, sessions) = setup_world();
    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 1);
    // content global is not set when register_content is never called
    assert_eq!(outputs[0].text, "no_content");
}

/// Test: Content with nested objects and various JSON types
#[test]
fn test_content_complex_values() {
    let dir = make_temp_dir("complex_values");
    let json = r#"[{
        "id": "dragon",
        "name": "Red Dragon",
        "stats": {"hp": 1000, "attack": 50, "defense": 30},
        "loot": ["gold", "dragon_scale"],
        "boss": true
    }]"#;
    fs::write(dir.join("monsters.json"), json).unwrap();

    let registry = ContentRegistry::load_dir(&dir).unwrap();
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine.register_content(&registry).unwrap();

    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                local d = content.monsters.dragon
                output:send(1, d.name)
                output:send(1, tostring(d.stats.hp))
                output:send(1, tostring(d.stats.attack))
                output:send(1, d.loot[1] .. "," .. d.loot[2])
                output:send(1, tostring(d.boss))
            end)
        "#,
        )
        .unwrap();

    let (mut ecs, mut space, sessions) = setup_world();
    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 5);
    assert_eq!(outputs[0].text, "Red Dragon");
    assert_eq!(outputs[1].text, "1000");
    assert_eq!(outputs[2].text, "50");
    assert_eq!(outputs[3].text, "gold,dragon_scale");
    assert_eq!(outputs[4].text, "true");

    let _ = fs::remove_dir_all(&dir);
}

/// Test: Content with GridSpace (grid mode compatibility)
#[test]
fn test_content_with_grid_space() {
    use space::grid_space::{GridConfig, GridSpace};

    let dir = make_temp_dir("grid_content");
    fs::write(
        dir.join("npcs.json"),
        r#"[{"id": "merchant", "name": "Merchant", "x": 5, "y": 5}]"#,
    )
    .unwrap();

    let registry = ContentRegistry::load_dir(&dir).unwrap();
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine.register_content(&registry).unwrap();

    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                local m = content.npcs.merchant
                output:send(1, m.name .. "@" .. tostring(m.x) .. "," .. tostring(m.y))
            end)
        "#,
        )
        .unwrap();

    let mut ecs = EcsAdapter::new();
    let mut grid = GridSpace::new(GridConfig {
        width: 20,
        height: 20,
        origin_x: 0,
        origin_y: 0,
    });
    let sessions = SessionManager::new();

    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut grid,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].text, "Merchant@5,5");

    let _ = fs::remove_dir_all(&dir);
}
