use std::collections::BTreeMap;

use ecs_adapter::EntityId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u64);

#[derive(Debug, Clone)]
pub struct SessionOutput {
    pub session_id: SessionId,
    pub text: String,
}

impl SessionOutput {
    pub fn new(session_id: SessionId, text: impl Into<String>) -> Self {
        Self {
            session_id,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    AwaitingLogin,
    Playing,
    Disconnected,
}

#[derive(Debug, Clone)]
pub struct PlayerSession {
    pub session_id: SessionId,
    pub state: SessionState,
    pub entity: Option<EntityId>,
    pub player_name: Option<String>,
}

impl PlayerSession {
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            state: SessionState::AwaitingLogin,
            entity: None,
            player_name: None,
        }
    }
}

/// Manages active player sessions.
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: BTreeMap<SessionId, PlayerSession>,
    entity_to_session: BTreeMap<EntityId, SessionId>,
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
}
