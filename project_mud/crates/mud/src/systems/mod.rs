use ecs_adapter::{EcsAdapter, EntityId};
use scripting::engine::{ActionInfo, ScriptContext, ScriptEngine};
use session::SessionId;
use space::RoomGraphSpace;

use crate::output::SessionOutput;
use crate::parser::PlayerAction;
use crate::session::SessionManager;

/// Type alias for MUD-specific ScriptContext (always RoomGraphSpace).
type MudScriptContext<'a> = ScriptContext<'a, RoomGraphSpace>;

/// Input from a player for this tick.
#[derive(Debug)]
pub struct PlayerInput {
    pub session_id: SessionId,
    pub entity: EntityId,
    pub action: PlayerAction,
}

/// Context passed to game systems.
pub struct GameContext<'a> {
    pub ecs: &'a mut EcsAdapter,
    pub space: &'a mut RoomGraphSpace,
    pub sessions: &'a SessionManager,
    pub tick: u64,
}

/// Process all player inputs via Lua on_action hooks, returning outputs.
pub fn run_game_systems(
    ctx: &mut GameContext<'_>,
    inputs: Vec<PlayerInput>,
    script_engine: Option<&ScriptEngine>,
) -> Vec<SessionOutput> {
    let mut outputs = Vec::new();

    for input in inputs {
        if let Some(engine) = script_engine {
            let (action_name, args) = action_to_lua_info(&input.action);
            let action_info = ActionInfo {
                action_name: action_name.clone(),
                args,
                session_id: input.session_id,
                entity: input.entity,
            };

            let mut script_ctx: MudScriptContext<'_> = ScriptContext {
                ecs: ctx.ecs,
                space: ctx.space,
                sessions: ctx.sessions,
                tick: ctx.tick,
            };

            match engine.run_on_action(&mut script_ctx, &action_info) {
                Ok((script_outputs, consumed)) => {
                    outputs.extend(script_outputs);
                    if consumed {
                        continue;
                    }
                }
                Err(e) => {
                    tracing::warn!("Script on_action error for '{}': {}", action_name, e);
                }
            }
        }

        // Fallback: if no script engine or script didn't consume
        outputs.push(SessionOutput::new(
            input.session_id,
            format!("알 수 없는 명령어: {:?}", input.action),
        ));
    }

    outputs
}

/// Convert a PlayerAction to a Lua action name and args string.
fn action_to_lua_info(action: &PlayerAction) -> (String, String) {
    match action {
        PlayerAction::Look => ("look".to_string(), String::new()),
        PlayerAction::Move(dir) => ("move".to_string(), format!("{:?}", dir).to_lowercase()),
        PlayerAction::Attack(target) => ("attack".to_string(), target.clone()),
        PlayerAction::Get(item) => ("get".to_string(), item.clone()),
        PlayerAction::Drop(item) => ("drop".to_string(), item.clone()),
        PlayerAction::InventoryList => ("inventory".to_string(), String::new()),
        PlayerAction::Say(msg) => ("say".to_string(), msg.clone()),
        PlayerAction::Who => ("who".to_string(), String::new()),
        PlayerAction::Quit => ("quit".to_string(), String::new()),
        PlayerAction::Help => ("help".to_string(), String::new()),
        PlayerAction::Admin { ref command, ref args } => ("admin".to_string(), format!("{} {}", command, args)),
        PlayerAction::Unknown(text) => ("unknown".to_string(), text.clone()),
    }
}
