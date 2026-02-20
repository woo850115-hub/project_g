use serde::{Deserialize, Serialize};

/// Client-to-server message (internally tagged JSON).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Connect { name: String },
    Move { dx: i32, dy: i32 },
    Action { name: String, args: Option<String> },
    Ping,
}

/// Server-to-client message (internally tagged JSON).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome {
        session_id: u64,
        entity_id: u64,
        tick: u64,
        grid_config: GridConfigWire,
    },
    EntityUpdate {
        tick: u64,
        entities: Vec<EntityWire>,
    },
    EntityRemove {
        tick: u64,
        entity_ids: Vec<u64>,
    },
    StateDelta {
        tick: u64,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        entered: Vec<EntityWire>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        moved: Vec<EntityMovedWire>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        left: Vec<u64>,
    },
    Error {
        message: String,
    },
    Pong,
}

/// Wire representation of an entity's position.
#[derive(Debug, Clone, Serialize)]
pub struct EntityWire {
    pub id: u64,
    pub x: i32,
    pub y: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub is_self: bool,
}

/// Wire representation of a moved entity (minimal: id + new position).
#[derive(Debug, Clone, Serialize)]
pub struct EntityMovedWire {
    pub id: u64,
    pub x: i32,
    pub y: i32,
}

/// Wire representation of grid configuration.
#[derive(Debug, Clone, Serialize)]
pub struct GridConfigWire {
    pub width: u32,
    pub height: u32,
    pub origin_x: i32,
    pub origin_y: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_connect() {
        let json = r#"{"type":"connect","name":"Player1"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Connect { name } => assert_eq!(name, "Player1"),
            _ => panic!("Expected Connect"),
        }
    }

    #[test]
    fn deserialize_move() {
        let json = r#"{"type":"move","dx":1,"dy":-1}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Move { dx, dy } => {
                assert_eq!(dx, 1);
                assert_eq!(dy, -1);
            }
            _ => panic!("Expected Move"),
        }
    }

    #[test]
    fn deserialize_action() {
        let json = r#"{"type":"action","name":"attack","args":"goblin"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Action { name, args } => {
                assert_eq!(name, "attack");
                assert_eq!(args.as_deref(), Some("goblin"));
            }
            _ => panic!("Expected Action"),
        }
    }

    #[test]
    fn deserialize_action_no_args() {
        let json = r#"{"type":"action","name":"look"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Action { name, args } => {
                assert_eq!(name, "look");
                assert!(args.is_none());
            }
            _ => panic!("Expected Action"),
        }
    }

    #[test]
    fn deserialize_ping() {
        let json = r#"{"type":"ping"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::Ping));
    }

    #[test]
    fn serialize_welcome() {
        let msg = ServerMessage::Welcome {
            session_id: 1_000_000,
            entity_id: 42,
            tick: 0,
            grid_config: GridConfigWire {
                width: 256,
                height: 256,
                origin_x: 0,
                origin_y: 0,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"welcome""#));
        assert!(json.contains(r#""session_id":1000000"#));
        assert!(json.contains(r#""entity_id":42"#));
    }

    #[test]
    fn serialize_entity_update() {
        let msg = ServerMessage::EntityUpdate {
            tick: 10,
            entities: vec![
                EntityWire {
                    id: 1,
                    x: 128,
                    y: 128,
                    name: Some("Player1".to_string()),
                    is_self: true,
                },
                EntityWire {
                    id: 2,
                    x: 100,
                    y: 100,
                    name: None,
                    is_self: false,
                },
            ],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"entity_update""#));
        assert!(json.contains(r#""is_self":true"#));
        // name:null should be skipped for entity 2
        assert!(!json.contains(r#""name":null"#));
    }

    #[test]
    fn serialize_entity_remove() {
        let msg = ServerMessage::EntityRemove {
            tick: 5,
            entity_ids: vec![10, 20],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"entity_remove""#));
        assert!(json.contains("[10,20]"));
    }

    #[test]
    fn serialize_error() {
        let msg = ServerMessage::Error {
            message: "out of bounds".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"error""#));
        assert!(json.contains("out of bounds"));
    }

    #[test]
    fn serialize_pong() {
        let msg = ServerMessage::Pong;
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"pong"}"#);
    }

    #[test]
    fn serialize_state_delta_full() {
        let msg = ServerMessage::StateDelta {
            tick: 42,
            entered: vec![EntityWire {
                id: 123,
                x: 50,
                y: 50,
                name: Some("Alice".to_string()),
                is_self: true,
            }],
            moved: vec![EntityMovedWire {
                id: 456,
                x: 51,
                y: 50,
            }],
            left: vec![789],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"state_delta""#));
        assert!(json.contains(r#""tick":42"#));
        assert!(json.contains(r#""entered""#));
        assert!(json.contains(r#""moved""#));
        assert!(json.contains(r#""left":[789]"#));
    }

    #[test]
    fn serialize_state_delta_entered_only() {
        let msg = ServerMessage::StateDelta {
            tick: 10,
            entered: vec![EntityWire {
                id: 1,
                x: 10,
                y: 20,
                name: None,
                is_self: false,
            }],
            moved: vec![],
            left: vec![],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"state_delta""#));
        assert!(json.contains(r#""entered""#));
        assert!(!json.contains(r#""moved""#));
        assert!(!json.contains(r#""left""#));
    }

    #[test]
    fn serialize_state_delta_empty_skips() {
        let msg = ServerMessage::StateDelta {
            tick: 5,
            entered: vec![],
            moved: vec![],
            left: vec![],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"state_delta""#));
        assert!(json.contains(r#""tick":5"#));
        assert!(!json.contains(r#""entered""#));
        assert!(!json.contains(r#""moved""#));
        assert!(!json.contains(r#""left""#));
    }

    #[test]
    fn serialize_entity_moved_wire() {
        let wire = EntityMovedWire {
            id: 99,
            x: -5,
            y: 10,
        };
        let json = serde_json::to_string(&wire).unwrap();
        assert!(json.contains(r#""id":99"#));
        assert!(json.contains(r#""x":-5"#));
        assert!(json.contains(r#""y":10"#));
    }
}
