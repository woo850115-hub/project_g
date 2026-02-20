/// Integration tests for GridSpace + Lua scripting engine.
/// Verifies that Lua scripts can interact with GridSpace via the SpaceProxy enum.
use ecs_adapter::EcsAdapter;
use scripting::engine::{ScriptContext, ScriptEngine};
use scripting::ScriptConfig;
use session::SessionManager;
use space::grid_space::{GridConfig, GridSpace};

fn make_grid() -> GridSpace {
    GridSpace::new(GridConfig {
        width: 20,
        height: 20,
        origin_x: 0,
        origin_y: 0,
    })
}

#[test]
fn grid_lua_set_and_get_position() {
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                local eid = ecs:spawn()
                space:set_position(eid, 7, 3)
                local pos = space:get_position(eid)
                output:send(1, pos.x .. "," .. pos.y)
            end)
        "#,
        )
        .unwrap();

    let mut ecs = EcsAdapter::new();
    let mut grid = make_grid();
    let sessions = SessionManager::new();

    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut grid,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].text, "7,3");
}

#[test]
fn grid_lua_move_to() {
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine
        .load_script(
            "test",
            r#"
            local saved_eid = nil
            hooks.on_init(function()
                saved_eid = ecs:spawn()
                space:set_position(saved_eid, 5, 5)
                -- move east (5,5) -> (6,5)
                space:move_to(saved_eid, 6, 5)
                local pos = space:get_position(saved_eid)
                output:send(1, pos.x .. "," .. pos.y)
            end)
        "#,
        )
        .unwrap();

    let mut ecs = EcsAdapter::new();
    let mut grid = make_grid();
    let sessions = SessionManager::new();

    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut grid,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].text, "6,5");
}

#[test]
fn grid_lua_entities_in_radius() {
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                local e1 = ecs:spawn()
                local e2 = ecs:spawn()
                local e3 = ecs:spawn()
                local e4 = ecs:spawn()

                space:set_position(e1, 5, 5) -- center
                space:set_position(e2, 6, 5) -- distance 1
                space:set_position(e3, 7, 5) -- distance 2
                space:set_position(e4, 8, 5) -- distance 3, out of radius

                local nearby = space:entities_in_radius(5, 5, 2)
                output:send(1, tostring(#nearby))
            end)
        "#,
        )
        .unwrap();

    let mut ecs = EcsAdapter::new();
    let mut grid = make_grid();
    let sessions = SessionManager::new();

    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut grid,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].text, "3"); // e1, e2, e3
}

#[test]
fn grid_lua_in_bounds() {
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                local inside = space:in_bounds(10, 10)
                local outside = space:in_bounds(100, 100)
                output:send(1, tostring(inside) .. "," .. tostring(outside))
            end)
        "#,
        )
        .unwrap();

    let mut ecs = EcsAdapter::new();
    let mut grid = make_grid();
    let sessions = SessionManager::new();

    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut grid,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].text, "true,false");
}

#[test]
fn grid_lua_grid_config() {
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                local cfg = space:grid_config()
                output:send(1, cfg.width .. "x" .. cfg.height)
            end)
        "#,
        )
        .unwrap();

    let mut ecs = EcsAdapter::new();
    let mut grid = make_grid();
    let sessions = SessionManager::new();

    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut grid,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].text, "20x20");
}

#[test]
fn grid_lua_entity_count() {
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                output:send(1, "before:" .. tostring(space:entity_count()))
                local eid = ecs:spawn()
                space:set_position(eid, 0, 0)
                output:send(1, "after:" .. tostring(space:entity_count()))
            end)
        "#,
        )
        .unwrap();

    let mut ecs = EcsAdapter::new();
    let mut grid = make_grid();
    let sessions = SessionManager::new();

    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut grid,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].text, "before:0");
    assert_eq!(outputs[1].text, "after:1");
}

#[test]
fn grid_lua_entity_room_via_spacemodel() {
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                local eid = ecs:spawn()
                space:set_position(eid, 3, 4)
                local room = space:entity_room(eid)
                -- room should be a non-nil number (synthetic cell EntityId)
                output:send(1, tostring(room ~= nil))
            end)
        "#,
        )
        .unwrap();

    let mut ecs = EcsAdapter::new();
    let mut grid = make_grid();
    let sessions = SessionManager::new();

    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut grid,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].text, "true");
}

#[test]
fn grid_lua_roomgraph_methods_error() {
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                -- room_count is RoomGraph-only, should error on Grid
                local ok, err = pcall(function()
                    return space:room_count()
                end)
                output:send(1, tostring(ok))
            end)
        "#,
        )
        .unwrap();

    let mut ecs = EcsAdapter::new();
    let mut grid = make_grid();
    let sessions = SessionManager::new();

    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut grid,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].text, "false"); // pcall returns false on error
}

#[test]
fn grid_lua_on_tick_moves_entity() {
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();

    // Store entity id as global via on_init
    engine
        .load_script(
            "test",
            r#"
            local entity_id = nil
            hooks.on_init(function()
                entity_id = ecs:spawn()
                space:set_position(entity_id, 0, 0)
            end)
            hooks.on_tick(function(tick)
                if entity_id then
                    local pos = space:get_position(entity_id)
                    if pos and pos.x < 19 then
                        space:move_to(entity_id, pos.x + 1, pos.y)
                    end
                end
            end)
        "#,
        )
        .unwrap();

    let mut ecs = EcsAdapter::new();
    let mut grid = make_grid();
    let sessions = SessionManager::new();

    // Run on_init to spawn and place entity
    {
        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut grid,
            sessions: &sessions,
            tick: 0,
        };
        engine.run_on_init(&mut ctx).unwrap();
    }

    // Run 5 ticks
    for tick in 1..=5 {
        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut grid,
            sessions: &sessions,
            tick,
        };
        engine.run_on_tick(&mut ctx).unwrap();
    }

    // Entity should have moved 5 cells to the right
    // Find the entity (spawned by Lua, should be the only one)
    assert_eq!(grid.entity_count(), 1);
    // We can check via the grid directly — there should be one entity at (5,0)
    let nearby = grid.entities_in_radius(5, 0, 0);
    assert_eq!(nearby.len(), 1);
}

#[test]
fn grid_lua_place_and_remove_entity() {
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    engine
        .load_script(
            "test",
            r#"
            hooks.on_init(function()
                local eid = ecs:spawn()
                space:set_position(eid, 10, 10)
                output:send(1, "count:" .. tostring(space:entity_count()))
                space:remove_entity(eid)
                output:send(1, "after_remove:" .. tostring(space:entity_count()))
            end)
        "#,
        )
        .unwrap();

    let mut ecs = EcsAdapter::new();
    let mut grid = make_grid();
    let sessions = SessionManager::new();

    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut grid,
        sessions: &sessions,
        tick: 0,
    };

    let outputs = engine.run_on_init(&mut ctx).unwrap();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].text, "count:1");
    assert_eq!(outputs[1].text, "after_remove:0");
}
