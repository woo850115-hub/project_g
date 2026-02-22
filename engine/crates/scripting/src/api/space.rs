use std::cell::RefCell;

use ecs_adapter::EntityId;
use mlua::{UserData, UserDataMethods};
use space::grid_space::GridSpace;
use space::model::SpaceModel;
use space::room_graph::RoomExits;
use space::RoomGraphSpace;

/// Which concrete space model backs this proxy.
#[doc(hidden)]
pub enum SpaceKind {
    RoomGraph(*mut RoomGraphSpace),
    Grid(*mut GridSpace),
}

/// Trait for converting a concrete space reference into a SpaceKind variant.
pub trait IntoSpaceKind: SpaceModel {
    fn into_space_kind(ptr: *mut Self) -> SpaceKind;
}

impl IntoSpaceKind for RoomGraphSpace {
    fn into_space_kind(ptr: *mut Self) -> SpaceKind {
        SpaceKind::RoomGraph(ptr)
    }
}

impl IntoSpaceKind for GridSpace {
    fn into_space_kind(ptr: *mut Self) -> SpaceKind {
        SpaceKind::Grid(ptr)
    }
}

/// Proxy object that Lua scripts use to access space operations.
pub struct SpaceProxy {
    space: RefCell<SpaceKind>,
}

// SAFETY: SpaceProxy is only used within a single tick-thread scope.
unsafe impl Send for SpaceProxy {}
unsafe impl Sync for SpaceProxy {}

impl SpaceProxy {
    /// Create a SpaceProxy from any concrete space model implementing IntoSpaceKind.
    ///
    /// # Safety
    /// Caller must ensure `space` outlives the proxy and is only used from one thread.
    pub unsafe fn from_space<S: IntoSpaceKind>(space: *mut S) -> Self {
        Self {
            space: RefCell::new(S::into_space_kind(space)),
        }
    }

    /// Access the space through the SpaceModel trait (works for both variants).
    fn with_model<R>(&self, f: impl FnOnce(&dyn SpaceModel) -> R) -> R {
        let kind = self.space.borrow();
        match &*kind {
            SpaceKind::RoomGraph(ptr) => f(unsafe { &**ptr }),
            SpaceKind::Grid(ptr) => f(unsafe { &**ptr }),
        }
    }

    /// Access the space mutably through the SpaceModel trait (works for both variants).
    fn with_model_mut<R>(&self, f: impl FnOnce(&mut dyn SpaceModel) -> R) -> R {
        let kind = self.space.borrow();
        match &*kind {
            SpaceKind::RoomGraph(ptr) => f(unsafe { &mut **ptr }),
            SpaceKind::Grid(ptr) => f(unsafe { &mut **ptr }),
        }
    }

    /// Access the space as a RoomGraphSpace. Returns Err for Grid mode.
    fn with_room_graph<R>(
        &self,
        f: impl FnOnce(&RoomGraphSpace) -> R,
    ) -> Result<R, mlua::Error> {
        let kind = self.space.borrow();
        match &*kind {
            SpaceKind::RoomGraph(ptr) => Ok(f(unsafe { &**ptr })),
            SpaceKind::Grid(_) => Err(mlua::Error::runtime(
                "this method is only available in RoomGraph mode",
            )),
        }
    }

    /// Access the space mutably as a RoomGraphSpace. Returns Err for Grid mode.
    fn with_room_graph_mut<R>(
        &self,
        f: impl FnOnce(&mut RoomGraphSpace) -> R,
    ) -> Result<R, mlua::Error> {
        let kind = self.space.borrow();
        match &*kind {
            SpaceKind::RoomGraph(ptr) => Ok(f(unsafe { &mut **ptr })),
            SpaceKind::Grid(_) => Err(mlua::Error::runtime(
                "this method is only available in RoomGraph mode",
            )),
        }
    }

    /// Access the space as a GridSpace. Returns Err for RoomGraph mode.
    fn with_grid<R>(&self, f: impl FnOnce(&GridSpace) -> R) -> Result<R, mlua::Error> {
        let kind = self.space.borrow();
        match &*kind {
            SpaceKind::Grid(ptr) => Ok(f(unsafe { &**ptr })),
            SpaceKind::RoomGraph(_) => Err(mlua::Error::runtime(
                "this method is only available in Grid mode",
            )),
        }
    }

    /// Access the space mutably as a GridSpace. Returns Err for RoomGraph mode.
    fn with_grid_mut<R>(&self, f: impl FnOnce(&mut GridSpace) -> R) -> Result<R, mlua::Error> {
        let kind = self.space.borrow();
        match &*kind {
            SpaceKind::Grid(ptr) => Ok(f(unsafe { &mut **ptr })),
            SpaceKind::RoomGraph(_) => Err(mlua::Error::runtime(
                "this method is only available in Grid mode",
            )),
        }
    }
}

impl UserData for SpaceProxy {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // ===== Common SpaceModel methods =====

        // space:entity_room(entity_id) -> room_id or nil
        methods.add_method("entity_room", |_lua, this, eid_u64: u64| {
            let eid = EntityId::from_u64(eid_u64);
            let result = this.with_model(|space| space.entity_room(eid));
            match result {
                Some(room) => Ok(Some(room.to_u64())),
                None => Ok(None),
            }
        });

        // space:move_entity(entity_id, target_room_id)
        methods.add_method("move_entity", |_lua, this, (eid_u64, target_u64): (u64, u64)| {
            let eid = EntityId::from_u64(eid_u64);
            let target = EntityId::from_u64(target_u64);
            this.with_model_mut(|space| space.move_entity(eid, target))
                .map_err(|e| mlua::Error::runtime(e.to_string()))?;
            Ok(())
        });

        // space:place_entity(entity_id, room_id)
        methods.add_method("place_entity", |_lua, this, (eid_u64, room_u64): (u64, u64)| {
            let eid = EntityId::from_u64(eid_u64);
            let room = EntityId::from_u64(room_u64);
            this.with_model_mut(|space| space.place_entity(eid, room))
                .map_err(|e| mlua::Error::runtime(e.to_string()))?;
            Ok(())
        });

        // space:remove_entity(entity_id)
        methods.add_method("remove_entity", |_lua, this, eid_u64: u64| {
            let eid = EntityId::from_u64(eid_u64);
            this.with_model_mut(|space| space.remove_entity(eid))
                .map_err(|e| mlua::Error::runtime(e.to_string()))?;
            Ok(())
        });

        // ===== RoomGraph-only methods =====

        // space:room_occupants(room_id) -> list of entity_ids
        methods.add_method("room_occupants", |_lua, this, room_u64: u64| {
            let room = EntityId::from_u64(room_u64);
            let occupants = this.with_room_graph(|space| space.room_occupants(room))?;
            let u64s: Vec<u64> = occupants.iter().map(|e| e.to_u64()).collect();
            Ok(u64s)
        });

        // space:register_room(entity_id, exits_table)
        methods.add_method("register_room", |_lua, this, (eid_u64, exits_table): (u64, mlua::Table)| {
            let room_id = EntityId::from_u64(eid_u64);
            let mut exits = RoomExits::default();

            if let Ok(n) = exits_table.get::<u64>("north") {
                exits.north = Some(EntityId::from_u64(n));
            }
            if let Ok(s) = exits_table.get::<u64>("south") {
                exits.south = Some(EntityId::from_u64(s));
            }
            if let Ok(e) = exits_table.get::<u64>("east") {
                exits.east = Some(EntityId::from_u64(e));
            }
            if let Ok(w) = exits_table.get::<u64>("west") {
                exits.west = Some(EntityId::from_u64(w));
            }

            this.with_room_graph_mut(|space| space.register_room(room_id, exits))?;
            Ok(())
        });

        // space:room_exists(room_id) -> bool
        methods.add_method("room_exists", |_lua, this, room_u64: u64| {
            let room = EntityId::from_u64(room_u64);
            this.with_room_graph(|space| space.room_exists(room))
        });

        // space:room_count() -> number
        methods.add_method("room_count", |_lua, this, ()| {
            this.with_room_graph(|space| space.room_count())
        });

        // space:all_rooms() -> list of entity_ids
        methods.add_method("all_rooms", |_lua, this, ()| {
            let rooms = this.with_room_graph(|space| space.all_rooms())?;
            let u64s: Vec<u64> = rooms.iter().map(|e| e.to_u64()).collect();
            Ok(u64s)
        });

        // space:exits(room_id) -> {north=id, south=id, ...} or nil
        methods.add_method("exits", |lua, this, room_u64: u64| {
            let room = EntityId::from_u64(room_u64);
            let exits = this.with_room_graph(|space| space.room_exits(room).cloned())?;
            match exits {
                Some(e) => {
                    let table = lua.create_table()?;
                    if let Some(n) = e.north {
                        table.set("north", n.to_u64())?;
                    }
                    if let Some(s) = e.south {
                        table.set("south", s.to_u64())?;
                    }
                    if let Some(e_exit) = e.east {
                        table.set("east", e_exit.to_u64())?;
                    }
                    if let Some(w) = e.west {
                        table.set("west", w.to_u64())?;
                    }
                    // Sort custom exits for determinism
                    let mut custom: Vec<_> = e.custom.into_iter().collect();
                    custom.sort_by(|a, b| a.0.cmp(&b.0));
                    for (name, id) in custom {
                        table.set(name, id.to_u64())?;
                    }
                    Ok(mlua::Value::Table(table))
                }
                None => Ok(mlua::Value::Nil),
            }
        });

        // ===== Grid-only methods =====

        // space:get_position(entity_id) -> {x=number, y=number} or nil
        methods.add_method("get_position", |lua, this, eid_u64: u64| {
            let eid = EntityId::from_u64(eid_u64);
            let pos = this.with_grid(|grid| grid.get_position(eid))?;
            match pos {
                Some(p) => {
                    let table = lua.create_table()?;
                    table.set("x", p.x)?;
                    table.set("y", p.y)?;
                    Ok(mlua::Value::Table(table))
                }
                None => Ok(mlua::Value::Nil),
            }
        });

        // space:set_position(entity_id, x, y)
        methods.add_method("set_position", |_lua, this, (eid_u64, x, y): (u64, i32, i32)| {
            let eid = EntityId::from_u64(eid_u64);
            this.with_grid_mut(|grid| grid.set_position(eid, x, y))?
                .map_err(|e| mlua::Error::runtime(e.to_string()))?;
            Ok(())
        });

        // space:move_to(entity_id, x, y) — adjacent move (Chebyshev distance 1)
        methods.add_method("move_to", |_lua, this, (eid_u64, x, y): (u64, i32, i32)| {
            let eid = EntityId::from_u64(eid_u64);
            this.with_grid_mut(|grid| grid.move_to(eid, x, y))?
                .map_err(|e| mlua::Error::runtime(e.to_string()))?;
            Ok(())
        });

        // space:entities_in_radius(x, y, radius) -> list of entity_ids
        methods.add_method("entities_in_radius", |_lua, this, (x, y, radius): (i32, i32, u32)| {
            let entities = this.with_grid(|grid| grid.entities_in_radius(x, y, radius))?;
            let u64s: Vec<u64> = entities.iter().map(|e| e.to_u64()).collect();
            Ok(u64s)
        });

        // space:in_bounds(x, y) -> bool
        methods.add_method("in_bounds", |_lua, this, (x, y): (i32, i32)| {
            this.with_grid(|grid| grid.in_bounds(x, y))
        });

        // space:grid_config() -> {width=number, height=number, origin_x=number, origin_y=number}
        methods.add_method("grid_config", |lua, this, ()| {
            let config = this.with_grid(|grid| grid.config().clone())?;
            let table = lua.create_table()?;
            table.set("width", config.width)?;
            table.set("height", config.height)?;
            table.set("origin_x", config.origin_x)?;
            table.set("origin_y", config.origin_y)?;
            Ok(table)
        });

        // space:entity_count() -> number
        methods.add_method("entity_count", |_lua, this, ()| {
            this.with_grid(|grid| grid.entity_count())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::{ScriptConfig, create_sandboxed_lua};
    use space::grid_space::GridConfig;
    use space::room_graph::RoomExits;

    fn setup_space() -> (RoomGraphSpace, EntityId, EntityId) {
        let mut space = RoomGraphSpace::new();
        let room_a = EntityId::new(100, 0);
        let room_b = EntityId::new(101, 0);

        space.register_room(room_a, RoomExits {
            north: Some(room_b),
            ..Default::default()
        });
        space.register_room(room_b, RoomExits {
            south: Some(room_a),
            ..Default::default()
        });

        (space, room_a, room_b)
    }

    fn setup_grid() -> GridSpace {
        GridSpace::new(GridConfig {
            width: 10,
            height: 10,
            origin_x: 0,
            origin_y: 0,
        })
    }

    #[test]
    fn test_space_entity_room() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let (mut space, room_a, _room_b) = setup_space();
        let entity = EntityId::new(1, 0);
        space.place_entity(entity, room_a).unwrap();

        let proxy = unsafe { SpaceProxy::from_space(&mut space as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            let room: u64 = lua.load(&format!(
                "return _space:entity_room({})", entity.to_u64()
            )).eval().unwrap();
            assert_eq!(room, room_a.to_u64());

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_space_room_occupants() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let (mut space, room_a, _room_b) = setup_space();
        let e1 = EntityId::new(1, 0);
        let e2 = EntityId::new(2, 0);
        space.place_entity(e1, room_a).unwrap();
        space.place_entity(e2, room_a).unwrap();

        let proxy = unsafe { SpaceProxy::from_space(&mut space as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            let occupants: Vec<u64> = lua.load(&format!(
                "return _space:room_occupants({})", room_a.to_u64()
            )).eval().unwrap();
            assert_eq!(occupants.len(), 2);

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_space_exits() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let (mut space, room_a, room_b) = setup_space();

        let proxy = unsafe { SpaceProxy::from_space(&mut space as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            let north: u64 = lua.load(&format!(
                "local e = _space:exits({}) return e.north", room_a.to_u64()
            )).eval().unwrap();
            assert_eq!(north, room_b.to_u64());

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_space_move_entity() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let (mut space, room_a, room_b) = setup_space();
        let entity = EntityId::new(1, 0);
        space.place_entity(entity, room_a).unwrap();

        let proxy = unsafe { SpaceProxy::from_space(&mut space as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            lua.load(&format!(
                "_space:move_entity({}, {})", entity.to_u64(), room_b.to_u64()
            )).exec().unwrap();

            let room: u64 = lua.load(&format!(
                "return _space:entity_room({})", entity.to_u64()
            )).eval().unwrap();
            assert_eq!(room, room_b.to_u64());

            Ok(())
        }).unwrap();
    }

    // ===== Grid-specific tests =====

    #[test]
    fn test_grid_get_set_position() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut grid = setup_grid();
        let entity = EntityId::new(1, 0);

        let proxy = unsafe { SpaceProxy::from_space(&mut grid as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            lua.load(&format!(
                "_space:set_position({}, 3, 4)", entity.to_u64()
            )).exec().unwrap();

            let result: mlua::Table = lua.load(&format!(
                "return _space:get_position({})", entity.to_u64()
            )).eval().unwrap();
            assert_eq!(result.get::<i32>("x").unwrap(), 3);
            assert_eq!(result.get::<i32>("y").unwrap(), 4);

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_grid_move_to() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut grid = setup_grid();
        let entity = EntityId::new(1, 0);
        grid.set_position(entity, 5, 5).unwrap();

        let proxy = unsafe { SpaceProxy::from_space(&mut grid as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            lua.load(&format!(
                "_space:move_to({}, 6, 5)", entity.to_u64()
            )).exec().unwrap();

            let result: mlua::Table = lua.load(&format!(
                "return _space:get_position({})", entity.to_u64()
            )).eval().unwrap();
            assert_eq!(result.get::<i32>("x").unwrap(), 6);
            assert_eq!(result.get::<i32>("y").unwrap(), 5);

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_grid_entities_in_radius() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut grid = setup_grid();
        let e1 = EntityId::new(1, 0);
        let e2 = EntityId::new(2, 0);
        let e3 = EntityId::new(3, 0);
        grid.set_position(e1, 5, 5).unwrap();
        grid.set_position(e2, 6, 5).unwrap();
        grid.set_position(e3, 9, 9).unwrap();

        let proxy = unsafe { SpaceProxy::from_space(&mut grid as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            let nearby: Vec<u64> = lua.load(
                "return _space:entities_in_radius(5, 5, 1)"
            ).eval().unwrap();
            assert_eq!(nearby.len(), 2);
            assert!(nearby.contains(&e1.to_u64()));
            assert!(nearby.contains(&e2.to_u64()));
            assert!(!nearby.contains(&e3.to_u64()));

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_grid_in_bounds() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut grid = setup_grid();

        let proxy = unsafe { SpaceProxy::from_space(&mut grid as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            let inside: bool = lua.load("return _space:in_bounds(5, 5)").eval().unwrap();
            assert!(inside);
            let outside: bool = lua.load("return _space:in_bounds(100, 100)").eval().unwrap();
            assert!(!outside);

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_grid_config() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut grid = setup_grid();

        let proxy = unsafe { SpaceProxy::from_space(&mut grid as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            let config: mlua::Table = lua.load("return _space:grid_config()").eval().unwrap();
            assert_eq!(config.get::<u32>("width").unwrap(), 10);
            assert_eq!(config.get::<u32>("height").unwrap(), 10);
            assert_eq!(config.get::<i32>("origin_x").unwrap(), 0);
            assert_eq!(config.get::<i32>("origin_y").unwrap(), 0);

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_grid_entity_count() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut grid = setup_grid();
        let e1 = EntityId::new(1, 0);
        grid.set_position(e1, 0, 0).unwrap();

        let proxy = unsafe { SpaceProxy::from_space(&mut grid as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            let count: usize = lua.load("return _space:entity_count()").eval().unwrap();
            assert_eq!(count, 1);

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_roomgraph_only_methods_fail_on_grid() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut grid = setup_grid();

        let proxy = unsafe { SpaceProxy::from_space(&mut grid as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            // room_occupants should fail on grid
            let result = lua.load("return _space:room_occupants(1)").exec();
            assert!(result.is_err());

            // room_count should fail on grid
            let result = lua.load("return _space:room_count()").exec();
            assert!(result.is_err());

            // register_room should fail on grid
            let result = lua.load("_space:register_room(1, {})").exec();
            assert!(result.is_err());

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_grid_only_methods_fail_on_roomgraph() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let (mut space, _room_a, _room_b) = setup_space();

        let proxy = unsafe { SpaceProxy::from_space(&mut space as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            // get_position should fail on RoomGraph
            let result = lua.load("return _space:get_position(1)").exec();
            assert!(result.is_err());

            // set_position should fail on RoomGraph
            let result = lua.load("_space:set_position(1, 0, 0)").exec();
            assert!(result.is_err());

            // grid_config should fail on RoomGraph
            let result = lua.load("return _space:grid_config()").exec();
            assert!(result.is_err());

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_grid_entity_room_via_spacemodel() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut grid = setup_grid();
        let entity = EntityId::new(1, 0);
        grid.set_position(entity, 3, 4).unwrap();

        let proxy = unsafe { SpaceProxy::from_space(&mut grid as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_space", ud).unwrap();

            // entity_room should work on Grid (returns synthetic cell EntityId)
            let room: u64 = lua.load(&format!(
                "return _space:entity_room({})", entity.to_u64()
            )).eval().unwrap();
            // Should be a valid cell EntityId
            assert!(room != 0);

            Ok(())
        }).unwrap();
    }
}
