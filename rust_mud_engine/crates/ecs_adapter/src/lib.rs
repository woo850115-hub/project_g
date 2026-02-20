pub mod types;
pub mod allocator;
pub mod bevy_backend;
pub mod error;

pub use types::{EntityId, ComponentId, AreaId, EventId};
pub use allocator::EntityAllocator;
pub use bevy_backend::EcsAdapter;
pub use error::EcsError;

pub use bevy_ecs::component::Component;
