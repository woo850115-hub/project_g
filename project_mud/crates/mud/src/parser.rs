use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    pub fn opposite(self) -> Self {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East => Direction::West,
            Direction::West => Direction::East,
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::North => write!(f, "북"),
            Direction::South => write!(f, "남"),
            Direction::East => write!(f, "동"),
            Direction::West => write!(f, "서"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerAction {
    Look,
    Move(Direction),
    Attack(String),
    Get(String),
    Drop(String),
    InventoryList,
    Say(String),
    Who,
    Quit,
    Help,
    Admin { command: String, args: String },
    Unknown(String),
}

/// Parse raw user input into a PlayerAction.
///
/// Format: `[argument] [command]` — the last word is the command, preceding words are the argument.
/// Single-word commands (e.g. "보기", "북", "도움말") work as before.
/// Admin commands (/command args) keep the original order.
pub fn parse_input(input: &str) -> PlayerAction {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return PlayerAction::Look;
    }

    // Admin commands start with / — keep [command] [args] order
    if trimmed.starts_with('/') {
        let without_slash = &trimmed[1..];
        let mut parts = without_slash.splitn(2, ' ');
        let command = parts.next().unwrap_or("").to_lowercase();
        let args = parts.next().unwrap_or("").trim().to_string();
        if command.is_empty() {
            return PlayerAction::Unknown("/".to_string());
        }
        return PlayerAction::Admin { command, args };
    }

    let lower = trimmed.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    if words.is_empty() {
        return PlayerAction::Look;
    }

    // Last word = command, preceding words = argument
    let cmd = words[words.len() - 1];
    let arg = if words.len() >= 2 {
        words[..words.len() - 1].join(" ")
    } else {
        String::new()
    };

    match cmd {
        // look  (ㅂ)
        "look" | "l" | "보기" | "\u{3142}" => PlayerAction::Look,
        // movement
        "north" | "n" | "북" => PlayerAction::Move(Direction::North),
        "south" | "s" | "남" => PlayerAction::Move(Direction::South),
        "east" | "e" | "동" => PlayerAction::Move(Direction::East),
        "west" | "w" | "서" => PlayerAction::Move(Direction::West),
        // attack  (ㄱ)
        "attack" | "kill" | "k" | "공격" | "\u{3131}" => {
            if arg.is_empty() {
                PlayerAction::Unknown("누구를 공격할까요?".to_string())
            } else {
                PlayerAction::Attack(arg)
            }
        }
        // get  (ㅈ)
        "get" | "take" | "pick" | "줍기" | "\u{3148}" => {
            if arg.is_empty() {
                PlayerAction::Unknown("무엇을 주울까요?".to_string())
            } else {
                PlayerAction::Get(arg)
            }
        }
        // drop  (ㅂㄹ)
        "drop" | "버리기" | "\u{3142}\u{3139}" => {
            if arg.is_empty() {
                PlayerAction::Unknown("무엇을 버릴까요?".to_string())
            } else {
                PlayerAction::Drop(arg)
            }
        }
        // inventory
        "inventory" | "inv" | "i" | "가방" | "인벤" => PlayerAction::InventoryList,
        // say  (ㅁ)
        "say" | "말" | "\u{3141}" => {
            if arg.is_empty() {
                PlayerAction::Unknown("무엇을 말할까요?".to_string())
            } else {
                PlayerAction::Say(arg)
            }
        }
        // who
        "who" | "접속자" => PlayerAction::Who,
        // quit
        "quit" | "exit" | "종료" => PlayerAction::Quit,
        // help  (ㄷ)
        "help" | "?" | "도움말" | "\u{3137}" => PlayerAction::Help,
        _ => PlayerAction::Unknown(trimmed.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_look() {
        assert_eq!(parse_input("보기"), PlayerAction::Look);
        assert_eq!(parse_input("\u{3142}"), PlayerAction::Look);
        assert_eq!(parse_input("look"), PlayerAction::Look);
        assert_eq!(parse_input("l"), PlayerAction::Look);
        assert_eq!(parse_input(""), PlayerAction::Look);
    }

    #[test]
    fn parse_movement() {
        assert_eq!(parse_input("북"), PlayerAction::Move(Direction::North));
        assert_eq!(parse_input("남"), PlayerAction::Move(Direction::South));
        assert_eq!(parse_input("동"), PlayerAction::Move(Direction::East));
        assert_eq!(parse_input("서"), PlayerAction::Move(Direction::West));
        assert_eq!(parse_input("north"), PlayerAction::Move(Direction::North));
        assert_eq!(parse_input("n"), PlayerAction::Move(Direction::North));
        assert_eq!(parse_input("south"), PlayerAction::Move(Direction::South));
        assert_eq!(parse_input("s"), PlayerAction::Move(Direction::South));
        assert_eq!(parse_input("east"), PlayerAction::Move(Direction::East));
        assert_eq!(parse_input("e"), PlayerAction::Move(Direction::East));
        assert_eq!(parse_input("west"), PlayerAction::Move(Direction::West));
        assert_eq!(parse_input("w"), PlayerAction::Move(Direction::West));
    }

    #[test]
    fn parse_attack() {
        // [arg] [cmd] format
        assert_eq!(parse_input("고블린 공격"), PlayerAction::Attack("고블린".to_string()));
        assert_eq!(parse_input("goblin attack"), PlayerAction::Attack("goblin".to_string()));
        assert_eq!(parse_input("goblin kill"), PlayerAction::Attack("goblin".to_string()));
        assert_eq!(parse_input("goblin k"), PlayerAction::Attack("goblin".to_string()));
        // Abbreviation: ㄱ
        assert_eq!(parse_input("고블린 \u{3131}"), PlayerAction::Attack("고블린".to_string()));
    }

    #[test]
    fn parse_attack_multi_word_target() {
        assert_eq!(
            parse_input("goblin warrior attack"),
            PlayerAction::Attack("goblin warrior".to_string()),
        );
        assert_eq!(
            parse_input("고블린 전사 공격"),
            PlayerAction::Attack("고블린 전사".to_string()),
        );
    }

    #[test]
    fn parse_attack_no_target() {
        assert_eq!(parse_input("공격"), PlayerAction::Unknown("누구를 공격할까요?".to_string()));
        assert_eq!(parse_input("attack"), PlayerAction::Unknown("누구를 공격할까요?".to_string()));
    }

    #[test]
    fn parse_get_drop() {
        // [arg] [cmd] format
        assert_eq!(parse_input("물약 줍기"), PlayerAction::Get("물약".to_string()));
        assert_eq!(parse_input("potion get"), PlayerAction::Get("potion".to_string()));
        assert_eq!(parse_input("sword take"), PlayerAction::Get("sword".to_string()));
        assert_eq!(parse_input("물약 버리기"), PlayerAction::Drop("물약".to_string()));
        assert_eq!(parse_input("potion drop"), PlayerAction::Drop("potion".to_string()));
        // Abbreviation: ㅈ for get, ㅂㄹ for drop
        assert_eq!(parse_input("물약 \u{3148}"), PlayerAction::Get("물약".to_string()));
        assert_eq!(parse_input("물약 \u{3142}\u{3139}"), PlayerAction::Drop("물약".to_string()));
    }

    #[test]
    fn parse_inventory() {
        assert_eq!(parse_input("가방"), PlayerAction::InventoryList);
        assert_eq!(parse_input("인벤"), PlayerAction::InventoryList);
        assert_eq!(parse_input("inventory"), PlayerAction::InventoryList);
        assert_eq!(parse_input("inv"), PlayerAction::InventoryList);
        assert_eq!(parse_input("i"), PlayerAction::InventoryList);
    }

    #[test]
    fn parse_say() {
        // [arg] [cmd] format
        assert_eq!(parse_input("안녕하세요 말"), PlayerAction::Say("안녕하세요".to_string()));
        assert_eq!(parse_input("hello world say"), PlayerAction::Say("hello world".to_string()));
        // Abbreviation: ㅁ
        assert_eq!(parse_input("안녕 \u{3141}"), PlayerAction::Say("안녕".to_string()));
    }

    #[test]
    fn parse_who_quit_help() {
        assert_eq!(parse_input("접속자"), PlayerAction::Who);
        assert_eq!(parse_input("who"), PlayerAction::Who);
        assert_eq!(parse_input("종료"), PlayerAction::Quit);
        assert_eq!(parse_input("quit"), PlayerAction::Quit);
        assert_eq!(parse_input("exit"), PlayerAction::Quit);
        assert_eq!(parse_input("도움말"), PlayerAction::Help);
        assert_eq!(parse_input("help"), PlayerAction::Help);
        assert_eq!(parse_input("?"), PlayerAction::Help);
        // Abbreviation: ㄷ for help
        assert_eq!(parse_input("\u{3137}"), PlayerAction::Help);
    }

    #[test]
    fn parse_unknown() {
        assert_eq!(parse_input("dance"), PlayerAction::Unknown("dance".to_string()));
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!(parse_input("NORTH"), PlayerAction::Move(Direction::North));
        assert_eq!(parse_input("Look"), PlayerAction::Look);
        // [arg] [cmd] format — arg is lowercased
        assert_eq!(parse_input("Goblin ATTACK"), PlayerAction::Attack("goblin".to_string()));
    }

    #[test]
    fn parse_whitespace_handling() {
        assert_eq!(parse_input("  north  "), PlayerAction::Move(Direction::North));
        assert_eq!(parse_input("  goblin   attack  "), PlayerAction::Attack("goblin".to_string()));
    }

    #[test]
    fn parse_admin_commands() {
        // Admin commands keep /command args order
        assert_eq!(
            parse_input("/kick TestUser"),
            PlayerAction::Admin {
                command: "kick".to_string(),
                args: "TestUser".to_string(),
            }
        );
        assert_eq!(
            parse_input("/announce Hello everyone!"),
            PlayerAction::Admin {
                command: "announce".to_string(),
                args: "Hello everyone!".to_string(),
            }
        );
        assert_eq!(
            parse_input("/stats"),
            PlayerAction::Admin {
                command: "stats".to_string(),
                args: String::new(),
            }
        );
        assert_eq!(
            parse_input("/TELEPORT Player 시작의 방"),
            PlayerAction::Admin {
                command: "teleport".to_string(),
                args: "Player 시작의 방".to_string(),
            }
        );
        assert_eq!(parse_input("/"), PlayerAction::Unknown("/".to_string()));
    }

    #[test]
    fn direction_opposite() {
        assert_eq!(Direction::North.opposite(), Direction::South);
        assert_eq!(Direction::South.opposite(), Direction::North);
        assert_eq!(Direction::East.opposite(), Direction::West);
        assert_eq!(Direction::West.opposite(), Direction::East);
    }

    #[test]
    fn direction_display() {
        assert_eq!(format!("{}", Direction::North), "북");
        assert_eq!(format!("{}", Direction::South), "남");
        assert_eq!(format!("{}", Direction::East), "동");
        assert_eq!(format!("{}", Direction::West), "서");
    }
}
