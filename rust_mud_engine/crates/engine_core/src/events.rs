use std::collections::HashMap;

use ecs_adapter::EventId;

/// Per-tick event bus with independent queues per EventId.
#[derive(Debug, Default)]
pub struct EventBus {
    queues: HashMap<EventId, Vec<Vec<u8>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            queues: HashMap::new(),
        }
    }

    /// Emit an event with a binary payload.
    pub fn emit(&mut self, event_id: EventId, payload: Vec<u8>) {
        self.queues.entry(event_id).or_default().push(payload);
    }

    /// Drain all events for a specific EventId.
    pub fn drain(&mut self, event_id: EventId) -> Vec<Vec<u8>> {
        self.queues.remove(&event_id).unwrap_or_default()
    }

    /// Drain all events from all queues (sorted by EventId for determinism).
    pub fn drain_all(&mut self) -> Vec<(EventId, Vec<Vec<u8>>)> {
        let mut entries: Vec<(EventId, Vec<Vec<u8>>)> = self.queues.drain().collect();
        entries.sort_by_key(|(id, _)| *id);
        entries
    }

    /// Clear all queues.
    pub fn clear(&mut self) {
        self.queues.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.queues.values().all(|q| q.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_and_drain() {
        let mut bus = EventBus::new();
        let eid = EventId(1);

        bus.emit(eid, vec![10, 20]);
        bus.emit(eid, vec![30, 40]);

        let events = bus.drain(eid);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], vec![10, 20]);
        assert_eq!(events[1], vec![30, 40]);

        // Drained, should be empty now
        let events = bus.drain(eid);
        assert!(events.is_empty());
    }

    #[test]
    fn independent_queues() {
        let mut bus = EventBus::new();
        bus.emit(EventId(1), vec![1]);
        bus.emit(EventId(2), vec![2]);

        let e1 = bus.drain(EventId(1));
        assert_eq!(e1.len(), 1);

        // EventId(2) should still be intact
        let e2 = bus.drain(EventId(2));
        assert_eq!(e2.len(), 1);
    }

    #[test]
    fn drain_all_sorted() {
        let mut bus = EventBus::new();
        bus.emit(EventId(3), vec![3]);
        bus.emit(EventId(1), vec![1]);
        bus.emit(EventId(2), vec![2]);

        let all = bus.drain_all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].0, EventId(1));
        assert_eq!(all[1].0, EventId(2));
        assert_eq!(all[2].0, EventId(3));
    }
}
