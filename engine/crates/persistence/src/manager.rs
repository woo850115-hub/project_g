use std::path::{Path, PathBuf};

use crate::error::PersistenceError;
use crate::snapshot::WorldSnapshot;

/// Manages snapshot persistence to disk.
pub struct SnapshotManager {
    save_dir: PathBuf,
}

impl SnapshotManager {
    pub fn new(save_dir: impl Into<PathBuf>) -> Self {
        Self {
            save_dir: save_dir.into(),
        }
    }

    /// Save a snapshot to disk.
    pub fn save_to_disk(&self, snapshot: &WorldSnapshot) -> Result<PathBuf, PersistenceError> {
        std::fs::create_dir_all(&self.save_dir)?;

        let filename = format!("snapshot_tick_{}.bin", snapshot.tick);
        let path = self.save_dir.join(&filename);

        let bytes = bincode::serialize(snapshot)?;

        // Write to temp file first, then rename for atomicity
        let tmp_path = self.save_dir.join(format!("{}.tmp", filename));
        std::fs::write(&tmp_path, &bytes)?;
        std::fs::rename(&tmp_path, &path)?;

        // Also update the "latest" symlink/file
        let latest_path = self.save_dir.join("latest.bin");
        let latest_tmp = self.save_dir.join("latest.bin.tmp");
        std::fs::write(&latest_tmp, &bytes)?;
        std::fs::rename(&latest_tmp, &latest_path)?;

        tracing::info!(
            tick = snapshot.tick,
            bytes = bytes.len(),
            path = %path.display(),
            "Snapshot saved"
        );

        Ok(path)
    }

    /// Load the latest snapshot from disk.
    pub fn load_latest(&self) -> Result<WorldSnapshot, PersistenceError> {
        let path = self.save_dir.join("latest.bin");
        self.load_from_path(&path)
    }

    /// Load a snapshot from a specific path.
    pub fn load_from_path(&self, path: &Path) -> Result<WorldSnapshot, PersistenceError> {
        let bytes = std::fs::read(path)?;
        let snapshot: WorldSnapshot = bincode::deserialize(&bytes)?;
        tracing::info!(
            tick = snapshot.tick,
            version = snapshot.version,
            path = %path.display(),
            "Snapshot loaded"
        );
        Ok(snapshot)
    }

    /// Check if a latest snapshot exists.
    pub fn has_latest(&self) -> bool {
        self.save_dir.join("latest.bin").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{PersistenceRegistry, PersistentComponent};
    use crate::snapshot;
    use ecs_adapter::{Component, EcsAdapter, EntityId};
    use serde::{Deserialize, Serialize};
    use space::RoomGraphSpace;

    #[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestName(String);

    struct TestNameHandler;
    impl PersistentComponent for TestNameHandler {
        fn tag(&self) -> &str {
            "TestName"
        }
        fn capture(&self, ecs: &EcsAdapter, eid: EntityId) -> Option<Vec<u8>> {
            ecs.get_component::<TestName>(eid)
                .ok()
                .and_then(|c| bincode::serialize(c).ok())
        }
        fn restore(
            &self,
            ecs: &mut EcsAdapter,
            eid: EntityId,
            data: &[u8],
        ) -> Result<(), crate::error::PersistenceError> {
            let c: TestName = bincode::deserialize(data)?;
            ecs.set_component(eid, c)
                .map_err(|e| crate::error::PersistenceError::Corrupt(e.to_string()))
        }
    }

    fn test_registry() -> PersistenceRegistry {
        let mut reg = PersistenceRegistry::new();
        reg.register(Box::new(TestNameHandler));
        reg
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("mud_test_persistence_save_load");
        let _ = std::fs::remove_dir_all(&dir);

        let registry = test_registry();
        let mut ecs = EcsAdapter::new();
        let space = RoomGraphSpace::new();

        let e1 = ecs.spawn_entity();
        ecs.set_component(e1, TestName("Hero".to_string())).unwrap();

        let snap = snapshot::capture(&ecs, &space, 42, &registry);
        let mgr = SnapshotManager::new(&dir);

        let path = mgr.save_to_disk(&snap).unwrap();
        assert!(path.exists());
        assert!(mgr.has_latest());

        let loaded = mgr.load_latest().unwrap();
        assert_eq!(loaded.tick, 42);
        assert_eq!(loaded.version, snap.version);
        assert_eq!(loaded.entities.len(), snap.entities.len());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_nonexistent_fails() {
        let dir = std::env::temp_dir().join("mud_test_persistence_nonexistent");
        let _ = std::fs::remove_dir_all(&dir);

        let mgr = SnapshotManager::new(&dir);
        assert!(!mgr.has_latest());
        assert!(mgr.load_latest().is_err());
    }

    #[test]
    fn multiple_snapshots() {
        let dir = std::env::temp_dir().join("mud_test_persistence_multiple");
        let _ = std::fs::remove_dir_all(&dir);

        let registry = test_registry();
        let mut ecs = EcsAdapter::new();
        let space = RoomGraphSpace::new();

        let e1 = ecs.spawn_entity();
        ecs.set_component(e1, TestName("Hero".to_string())).unwrap();

        let mgr = SnapshotManager::new(&dir);

        let snap1 = snapshot::capture(&ecs, &space, 100, &registry);
        mgr.save_to_disk(&snap1).unwrap();

        let snap2 = snapshot::capture(&ecs, &space, 200, &registry);
        mgr.save_to_disk(&snap2).unwrap();

        // Latest should be the most recent
        let loaded = mgr.load_latest().unwrap();
        assert_eq!(loaded.tick, 200);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
