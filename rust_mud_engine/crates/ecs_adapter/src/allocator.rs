use serde::{Deserialize, Serialize};

use crate::types::EntityId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityAllocator {
    generations: Vec<u32>,
    alive: Vec<bool>,
    free_indices: Vec<u32>,
    next_index: u32,
}

impl EntityAllocator {
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            alive: Vec::new(),
            free_indices: Vec::new(),
            next_index: 0,
        }
    }

    pub fn allocate(&mut self) -> EntityId {
        if let Some(index) = self.free_indices.pop() {
            let idx = index as usize;
            self.generations[idx] += 1;
            self.alive[idx] = true;
            EntityId::new(index, self.generations[idx])
        } else {
            let index = self.next_index;
            self.next_index += 1;
            self.generations.push(0);
            self.alive.push(true);
            EntityId::new(index, 0)
        }
    }

    pub fn deallocate(&mut self, id: EntityId) -> bool {
        let idx = id.index as usize;
        if idx >= self.alive.len() {
            return false;
        }
        if !self.alive[idx] || self.generations[idx] != id.generation {
            return false;
        }
        self.alive[idx] = false;
        self.free_indices.push(id.index);
        true
    }

    pub fn is_alive(&self, id: EntityId) -> bool {
        let idx = id.index as usize;
        idx < self.alive.len() && self.alive[idx] && self.generations[idx] == id.generation
    }

    pub fn alive_count(&self) -> usize {
        self.alive.iter().filter(|&&a| a).count()
    }
}

impl Default for EntityAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_returns_increasing_indices() {
        let mut alloc = EntityAllocator::new();
        let a = alloc.allocate();
        let b = alloc.allocate();
        assert_eq!(a.index, 0);
        assert_eq!(a.generation, 0);
        assert_eq!(b.index, 1);
        assert_eq!(b.generation, 0);
    }

    #[test]
    fn deallocate_and_reallocate_increments_generation() {
        let mut alloc = EntityAllocator::new();
        let a = alloc.allocate();
        assert!(alloc.is_alive(a));
        assert!(alloc.deallocate(a));
        assert!(!alloc.is_alive(a));

        let b = alloc.allocate();
        assert_eq!(b.index, a.index);
        assert_eq!(b.generation, a.generation + 1);
        assert!(alloc.is_alive(b));
        assert!(!alloc.is_alive(a));
    }

    #[test]
    fn double_deallocate_returns_false() {
        let mut alloc = EntityAllocator::new();
        let a = alloc.allocate();
        assert!(alloc.deallocate(a));
        assert!(!alloc.deallocate(a));
    }

    #[test]
    fn bincode_roundtrip() {
        let mut alloc = EntityAllocator::new();
        let _a = alloc.allocate();
        let b = alloc.allocate();
        alloc.deallocate(b);

        let bytes = bincode::serialize(&alloc).unwrap();
        let restored: EntityAllocator = bincode::deserialize(&bytes).unwrap();

        assert_eq!(alloc.alive_count(), restored.alive_count());
        assert_eq!(restored.alive_count(), 1);
    }
}
