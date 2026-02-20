use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("script load error: {0}")]
    Load(String),

    #[error("component not registered: {0}")]
    ComponentNotRegistered(String),

    #[error("sandbox violation: {0}")]
    Sandbox(String),

    #[error("memory limit exceeded")]
    MemoryLimitExceeded,

    #[error("instruction limit exceeded")]
    InstructionLimitExceeded,

    #[error("content load error: {0}")]
    ContentLoad(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
