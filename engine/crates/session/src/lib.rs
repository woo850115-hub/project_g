use std::collections::BTreeMap;

use ecs_adapter::EntityId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u64);

#[derive(Debug, Clone)]
pub struct SessionOutput {
    pub session_id: SessionId,
    pub text: String,
    /// When true, the output router will close the session's write channel
    /// after delivering this message, causing the TCP connection to shut down.
    pub disconnect: bool,
}

impl SessionOutput {
    pub fn new(session_id: SessionId, text: impl Into<String>) -> Self {
        Self {
            session_id,
            text: text.into(),
            disconnect: false,
        }
    }

    /// Create a final message that will disconnect the session after delivery.
    pub fn with_disconnect(session_id: SessionId, text: impl Into<String>) -> Self {
        Self {
            session_id,
            text: text.into(),
            disconnect: true,
        }
    }
}

/// Permission levels matching player_db::PermissionLevel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum PermissionLevel {
    Player = 0,
    Builder = 1,
    Admin = 2,
    Owner = 3,
}

impl PermissionLevel {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Builder,
            2 => Self::Admin,
            3 => Self::Owner,
            _ => Self::Player,
        }
    }

    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

impl Default for PermissionLevel {
    fn default() -> Self {
        Self::Player
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    AwaitingLogin,
    AwaitingPassword {
        username: String,
        is_new: bool,
    },
    AwaitingPasswordConfirm {
        username: String,
        password: String,
    },
    SelectingCharacter {
        account_id: i64,
        permission: PermissionLevel,
    },
    Playing,
    Disconnected,
}

#[derive(Debug, Clone)]
pub struct PlayerSession {
    pub session_id: SessionId,
    pub state: SessionState,
    pub entity: Option<EntityId>,
    pub player_name: Option<String>,
    pub account_id: Option<i64>,
    pub character_id: Option<i64>,
    pub permission: PermissionLevel,
}

impl PlayerSession {
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            state: SessionState::AwaitingLogin,
            entity: None,
            player_name: None,
            account_id: None,
            character_id: None,
            permission: PermissionLevel::Player,
        }
    }
}

/// A player entity that remains in-world after disconnect, awaiting reconnection.
#[derive(Debug, Clone)]
pub struct LingeringEntity {
    pub entity: EntityId,
    pub character_id: i64,
    pub account_id: i64,
    pub disconnect_tick: u64,
}

/// Manages active player sessions.
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: BTreeMap<SessionId, PlayerSession>,
    entity_to_session: BTreeMap<EntityId, SessionId>,
    lingering: BTreeMap<i64, LingeringEntity>, // character_id -> LingeringEntity
    next_id: u64,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new session, returning its ID.
    pub fn create_session(&mut self) -> SessionId {
        let id = SessionId(self.next_id);
        self.next_id += 1;
        self.sessions.insert(id, PlayerSession::new(id));
        id
    }

    /// Create a session with a specific ID (used when network layer assigns IDs).
    pub fn create_session_with_id(&mut self, id: SessionId) {
        self.sessions.insert(id, PlayerSession::new(id));
        if id.0 >= self.next_id {
            self.next_id = id.0 + 1;
        }
    }

    /// Get a session by ID.
    pub fn get_session(&self, id: SessionId) -> Option<&PlayerSession> {
        self.sessions.get(&id)
    }

    /// Get a mutable session by ID.
    pub fn get_session_mut(&mut self, id: SessionId) -> Option<&mut PlayerSession> {
        self.sessions.get_mut(&id)
    }

    /// Get session by entity.
    pub fn session_for_entity(&self, entity: EntityId) -> Option<&PlayerSession> {
        let sid = self.entity_to_session.get(&entity)?;
        self.sessions.get(sid)
    }

    /// Get session ID for an entity.
    pub fn session_id_for_entity(&self, entity: EntityId) -> Option<SessionId> {
        self.entity_to_session.get(&entity).copied()
    }

    /// Bind an entity to a session (on login).
    pub fn bind_entity(&mut self, session_id: SessionId, entity: EntityId) {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.entity = Some(entity);
            session.state = SessionState::Playing;
            self.entity_to_session.insert(entity, session_id);
        }
    }

    /// Mark a session as disconnected and remove entity mapping.
    pub fn disconnect(&mut self, session_id: SessionId) -> Option<EntityId> {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.state = SessionState::Disconnected;
            let entity = session.entity.take();
            if let Some(eid) = entity {
                self.entity_to_session.remove(&eid);
            }
            return entity;
        }
        None
    }

    /// Remove a disconnected session entirely.
    pub fn remove_session(&mut self, session_id: SessionId) {
        if let Some(session) = self.sessions.remove(&session_id) {
            if let Some(eid) = session.entity {
                self.entity_to_session.remove(&eid);
            }
        }
    }

    /// All sessions in Playing state (sorted by session ID).
    pub fn playing_sessions(&self) -> Vec<&PlayerSession> {
        self.sessions
            .values()
            .filter(|s| s.state == SessionState::Playing)
            .collect()
    }

    /// All session IDs.
    pub fn all_session_ids(&self) -> Vec<SessionId> {
        self.sessions.keys().copied().collect()
    }

    /// Count of active (non-disconnected) sessions.
    pub fn active_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|s| s.state != SessionState::Disconnected)
            .count()
    }

    /// Add a lingering entity (stays in-world after disconnect).
    pub fn add_lingering(&mut self, linger: LingeringEntity) {
        self.lingering.insert(linger.character_id, linger);
    }

    /// Find a lingering entity by character ID.
    pub fn find_lingering(&self, character_id: i64) -> Option<&LingeringEntity> {
        self.lingering.get(&character_id)
    }

    /// Remove and return a lingering entity by character ID.
    pub fn remove_lingering(&mut self, character_id: i64) -> Option<LingeringEntity> {
        self.lingering.remove(&character_id)
    }

    /// Return character IDs of lingering entities that have exceeded the timeout.
    pub fn expired_lingering(&self, current_tick: u64, timeout_ticks: u64) -> Vec<i64> {
        self.lingering
            .iter()
            .filter(|(_, l)| current_tick.saturating_sub(l.disconnect_tick) >= timeout_ticks)
            .map(|(id, _)| *id)
            .collect()
    }

    /// All lingering entities (for batch operations like auto-save).
    pub fn lingering_entities(&self) -> Vec<&LingeringEntity> {
        self.lingering.values().collect()
    }

    /// Rebind a lingering entity to a new session (seamless reconnection).
    pub fn rebind_lingering(&mut self, session_id: SessionId, character_id: i64) -> Option<EntityId> {
        let linger = self.lingering.remove(&character_id)?;
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.entity = Some(linger.entity);
            session.state = SessionState::Playing;
            session.character_id = Some(character_id);
            session.account_id = Some(linger.account_id);
            self.entity_to_session.insert(linger.entity, session_id);
        }
        Some(linger.entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_session_increments_id() {
        let mut mgr = SessionManager::new();
        let s1 = mgr.create_session();
        let s2 = mgr.create_session();
        assert_eq!(s1, SessionId(0));
        assert_eq!(s2, SessionId(1));
    }

    #[test]
    fn session_lifecycle() {
        let mut mgr = SessionManager::new();
        let sid = mgr.create_session();

        // Initially awaiting login
        let session = mgr.get_session(sid).unwrap();
        assert_eq!(session.state, SessionState::AwaitingLogin);
        assert!(session.entity.is_none());

        // Bind entity
        let eid = EntityId::new(1, 0);
        mgr.bind_entity(sid, eid);
        let session = mgr.get_session(sid).unwrap();
        assert_eq!(session.state, SessionState::Playing);
        assert_eq!(session.entity, Some(eid));

        // Entity lookup
        assert_eq!(mgr.session_id_for_entity(eid), Some(sid));

        // Disconnect
        let removed = mgr.disconnect(sid);
        assert_eq!(removed, Some(eid));
        let session = mgr.get_session(sid).unwrap();
        assert_eq!(session.state, SessionState::Disconnected);
        assert!(mgr.session_id_for_entity(eid).is_none());
    }

    #[test]
    fn playing_sessions_filter() {
        let mut mgr = SessionManager::new();
        let s1 = mgr.create_session();
        let _s2 = mgr.create_session();

        mgr.bind_entity(s1, EntityId::new(1, 0));
        // s2 still awaiting login

        let playing = mgr.playing_sessions();
        assert_eq!(playing.len(), 1);
        assert_eq!(playing[0].session_id, s1);
    }

    #[test]
    fn remove_session_cleans_up() {
        let mut mgr = SessionManager::new();
        let sid = mgr.create_session();
        let eid = EntityId::new(1, 0);
        mgr.bind_entity(sid, eid);
        mgr.remove_session(sid);

        assert!(mgr.get_session(sid).is_none());
        assert!(mgr.session_id_for_entity(eid).is_none());
    }

    #[test]
    fn active_count() {
        let mut mgr = SessionManager::new();
        let s1 = mgr.create_session();
        let _s2 = mgr.create_session();
        assert_eq!(mgr.active_count(), 2);

        mgr.disconnect(s1);
        assert_eq!(mgr.active_count(), 1);
    }

    #[test]
    fn session_state_new_variants() {
        let state = SessionState::AwaitingPassword {
            username: "test".into(),
            is_new: true,
        };
        assert!(matches!(state, SessionState::AwaitingPassword { is_new: true, .. }));

        let state = SessionState::SelectingCharacter {
            account_id: 42,
            permission: PermissionLevel::Admin,
        };
        assert!(matches!(state, SessionState::SelectingCharacter { account_id: 42, .. }));
    }

    #[test]
    fn session_fields() {
        let mut mgr = SessionManager::new();
        let sid = mgr.create_session();
        let session = mgr.get_session_mut(sid).unwrap();
        session.account_id = Some(1);
        session.character_id = Some(10);
        session.permission = PermissionLevel::Builder;

        let session = mgr.get_session(sid).unwrap();
        assert_eq!(session.account_id, Some(1));
        assert_eq!(session.character_id, Some(10));
        assert_eq!(session.permission, PermissionLevel::Builder);
    }

    #[test]
    fn permission_level_ordering() {
        assert!(PermissionLevel::Player < PermissionLevel::Builder);
        assert!(PermissionLevel::Builder < PermissionLevel::Admin);
        assert!(PermissionLevel::Admin < PermissionLevel::Owner);
    }

    #[test]
    fn lingering_add_find_remove() {
        let mut mgr = SessionManager::new();
        let eid = EntityId::new(5, 0);

        mgr.add_lingering(LingeringEntity {
            entity: eid,
            character_id: 42,
            account_id: 1,
            disconnect_tick: 100,
        });

        assert!(mgr.find_lingering(42).is_some());
        assert_eq!(mgr.find_lingering(42).unwrap().entity, eid);
        assert!(mgr.find_lingering(99).is_none());

        let removed = mgr.remove_lingering(42);
        assert!(removed.is_some());
        assert!(mgr.find_lingering(42).is_none());
    }

    #[test]
    fn lingering_expired() {
        let mut mgr = SessionManager::new();
        mgr.add_lingering(LingeringEntity {
            entity: EntityId::new(1, 0),
            character_id: 10,
            account_id: 1,
            disconnect_tick: 100,
        });
        mgr.add_lingering(LingeringEntity {
            entity: EntityId::new(2, 0),
            character_id: 20,
            account_id: 2,
            disconnect_tick: 200,
        });

        // At tick 250, timeout 100: character 10 expired (250-100=150 >= 100), character 20 not (250-200=50 < 100)
        let expired = mgr.expired_lingering(250, 100);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0], 10);

        // At tick 350, both expired
        let expired = mgr.expired_lingering(350, 100);
        assert_eq!(expired.len(), 2);
    }

    #[test]
    fn rebind_lingering_to_new_session() {
        let mut mgr = SessionManager::new();
        let eid = EntityId::new(5, 0);

        mgr.add_lingering(LingeringEntity {
            entity: eid,
            character_id: 42,
            account_id: 1,
            disconnect_tick: 100,
        });

        let sid = mgr.create_session();
        let result = mgr.rebind_lingering(sid, 42);
        assert_eq!(result, Some(eid));

        let session = mgr.get_session(sid).unwrap();
        assert_eq!(session.state, SessionState::Playing);
        assert_eq!(session.entity, Some(eid));
        assert_eq!(session.character_id, Some(42));
        assert_eq!(mgr.session_id_for_entity(eid), Some(sid));

        // Lingering entry removed
        assert!(mgr.find_lingering(42).is_none());
    }
}
