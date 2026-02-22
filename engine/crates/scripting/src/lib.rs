pub mod error;
pub mod sandbox;
pub mod hooks;
pub mod engine;
pub mod component_registry;
pub mod api;
pub mod template;
pub mod content;

pub use engine::ScriptEngine;
pub use error::ScriptError;
pub use sandbox::ScriptConfig;
pub use hooks::HookRegistry;
pub use content::ContentRegistry;

// Re-export mlua for downstream crates implementing ScriptComponent
pub use mlua;
