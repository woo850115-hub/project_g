# Phase 3: Lua Scripting Engine (WASM → Luau Transition)

## Overview

Phase 3 replaces the "blind" WASM command generator with a full Luau scripting engine.
Lua scripts run on the tick thread with direct ECS/Space read/write access.

## Architecture

```
engine_core → ecs_adapter, space, observability, plugin_abi, plugin_runtime
scripting   → ecs_adapter, space, session, mlua (Luau VM)
mud         → ecs_adapter, space, session, persistence, scripting
```

## New Crate: `crates/scripting/`

### Structure

```
crates/scripting/
├── Cargo.toml
└── src/
    ├── lib.rs                — pub mod declarations, re-exports
    ├── engine.rs             — ScriptEngine: Lua VM management, script loading, hook execution
    ├── sandbox.rs            — ScriptConfig, Luau sandbox (memory/instruction limits)
    ├── component_registry.rs — ScriptComponent trait, ScriptComponentRegistry
    ├── hooks.rs              — HookRegistry: per-event Lua callback management
    ├── template.rs           — GameTemplate loader (game.toml + scripts/)
    ├── error.rs              — ScriptError enum
    └── api/
        ├── mod.rs
        ├── ecs.rs            — ecs:get/set/has/remove/spawn/despawn/query (via EcsProxy)
        ├── space.rs          — space:entity_room/room_occupants/move_entity/exits (via SpaceProxy)
        ├── output.rs         — output:send/broadcast_room (via OutputProxy)
        └── log.rs            — log.info/warn/error/debug (maps to tracing)
```

### Key Types

- `ScriptEngine` — Main entry point. Manages Lua VM, loads scripts, executes hooks.
- `ScriptConfig` — Sandbox settings (memory_limit, instruction_limit).
- `ScriptContext<'a>` — Mutable references to ECS/Space/Sessions for hook execution.
- `ActionInfo` — Player action metadata for on_action hooks.
- `ScriptComponentRegistry` — Maps string tags to `ScriptComponent` trait objects.
- `HookRegistry` — Stores Lua callback `RegistryKey`s per event type.
- `EcsProxy/SpaceProxy/OutputProxy` — UserData wrappers using raw pointers (safe within scope).

### Proxy Pattern

All proxies use `RefCell<*mut T>` with `unsafe impl Send + Sync`.
Safety is guaranteed by:
1. Proxies are created within `lua.scope()` and dropped when scope ends.
2. Only used from the tick thread (single writer thread invariant).
3. Lifetime of underlying data exceeds the scope.

### Sandbox

- Memory limit: 16 MB (configurable)
- Instruction limit: 1,000,000 per execution batch (configurable)
- Luau sandbox mode: restricts os/io/loadfile access
- Instruction counter reset before each hook execution batch

## Lua API Surface

```lua
-- ECS
ecs:get(entity_id, "Health")        -- returns table or nil
ecs:set(entity_id, "Health", {current=70, max=100})
ecs:has(entity_id, "PlayerTag")     -- returns bool
ecs:remove(entity_id, "Dead")
ecs:spawn()                         -- returns entity_id (u64)
ecs:despawn(entity_id)
ecs:query("Health", "NpcTag")       -- returns list of entity_ids

-- Space
space:entity_room(entity_id)        -- returns room_id or nil
space:room_occupants(room_id)       -- returns list of entity_ids
space:move_entity(entity_id, room)  -- moves (requires neighbor check)
space:place_entity(entity_id, room) -- initial placement
space:remove_entity(entity_id)
space:exits(room_id)                -- returns {north=id, south=id, ...}

-- Output
output:send(session_id, "text")
output:broadcast_room(room_id, "text", {exclude=entity_id})

-- Hooks
hooks.on_tick(function(tick) end)
hooks.on_action("attack", function(ctx) return true end)  -- return true to consume
hooks.on_enter_room(function(entity, room, old_room) end)
hooks.on_connect(function(session_id) end)

-- Logging
log.info("message")
log.warn("message")
log.error("message")
log.debug("message")
```

## Tick Loop Integration

```
main.rs tick loop:
1. Network messages (try_recv)
2. tick_loop.step()           — WASM plugins + command stream
3. script_engine.run_on_tick() — Lua periodic hooks (NEW)
4. run_game_systems()         — Player action processing (with on_action hooks) (MODIFIED)
5. Output send
6. Snapshot
```

## Game Template System

```
games/my_game/
├── game.toml           — name, version, description, scripts list
└── scripts/
    ├── world_setup.lua
    ├── combat.lua
    └── commands.lua
```

- If `scripts` list in game.toml is non-empty, load in specified order.
- If empty, auto-discover all .lua/.luau files alphabetically.

## MUD Integration

- `mud/src/script_setup.rs` — Registers all 12 MUD components with ScriptComponentRegistry.
- `mud/src/systems/mod.rs` — `run_game_systems()` now accepts `Option<&ScriptEngine>`.
  Before default action processing, on_action hooks are called.
  If any hook returns `true`, the action is consumed (skips default handler).

## Test Summary

| Module | Tests |
|--------|-------|
| sandbox | 4 (VM creation, sandbox restrictions, memory limit, config) |
| engine | 12 (new, load, hooks, run_on_tick, run_on_action, run_on_enter_room, run_on_connect, ECS access) |
| hooks | 1 (registry creation) |
| component_registry | 1 (empty registry) |
| api/ecs | 5 (get/set roundtrip, has, spawn/despawn, query, nil for missing) |
| api/space | 4 (entity_room, room_occupants, exits, move_entity) |
| api/output | 2 (send, broadcast_room) |
| api/log | 1 (all log levels) |
| template | 4 (load, scripts, no toml, auto-discovery) |
| **Total scripting** | **36** |
| **Total workspace** | **154** |

## Dependencies Added

```toml
mlua = { version = "0.10", features = ["luau", "vendored", "send", "serialize"] }
serde_json = "1"
toml = "0.8"
```
