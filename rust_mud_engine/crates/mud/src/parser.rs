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
    Unknown(String),
}

/// Parse raw user input into a PlayerAction.
pub fn parse_input(input: &str) -> PlayerAction {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return PlayerAction::Look;
    }

    let lower = trimmed.to_lowercase();
    let mut parts = lower.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("");
    let arg = parts.next().unwrap_or("").trim().to_string();

    match cmd {
        "look" | "l" | "보기" | "\u{3142}" => PlayerAction::Look,
        "north" | "n" | "북" => PlayerAction::Move(Direction::North),
        "south" | "s" | "남" => PlayerAction::Move(Direction::South),
        "east" | "e" | "동" => PlayerAction::Move(Direction::East),
        "west" | "w" | "서" => PlayerAction::Move(Direction::West),
        "attack" | "kill" | "k" | "공격" => {
            if arg.is_empty() {
                PlayerAction::Unknown("누구를 공격할까요?".to_string())
            } else {
                PlayerAction::Attack(arg)
            }
        }
        "get" | "take" | "pick" | "줍기" => {
            if arg.is_empty() {
                PlayerAction::Unknown("무엇을 주울까요?".to_string())
            } else {
                PlayerAction::Get(arg)
            }
        }
        "drop" | "버리기" => {
            if arg.is_empty() {
                PlayerAction::Unknown("무엇을 버릴까요?".to_string())
            } else {
                PlayerAction::Drop(arg)
            }
        }
        "inventory" | "inv" | "i" | "가방" | "인벤" => PlayerAction::InventoryList,
        "say" | "말" => {
            if arg.is_empty() {
                PlayerAction::Unknown("무엇을 말할까요?".to_string())
            } else {
                PlayerAction::Say(arg)
            }
        }
        "who" | "접속자" => PlayerAction::Who,
        "quit" | "exit" | "종료" => PlayerAction::Quit,
        "help" | "?" | "도움말" => PlayerAction::Help,
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
        assert_eq!(parse_input("공격 고블린"), PlayerAction::Attack("고블린".to_string()));
        assert_eq!(parse_input("attack goblin"), PlayerAction::Attack("goblin".to_string()));
        assert_eq!(parse_input("kill goblin"), PlayerAction::Attack("goblin".to_string()));
        assert_eq!(parse_input("k goblin"), PlayerAction::Attack("goblin".to_string()));
    }

    #[test]
    fn parse_attack_no_target() {
        assert_eq!(parse_input("공격"), PlayerAction::Unknown("누구를 공격할까요?".to_string()));
        assert_eq!(parse_input("attack"), PlayerAction::Unknown("누구를 공격할까요?".to_string()));
    }

    #[test]
    fn parse_get_drop() {
        assert_eq!(parse_input("줍기 물약"), PlayerAction::Get("물약".to_string()));
        assert_eq!(parse_input("get potion"), PlayerAction::Get("potion".to_string()));
        assert_eq!(parse_input("take sword"), PlayerAction::Get("sword".to_string()));
        assert_eq!(parse_input("버리기 물약"), PlayerAction::Drop("물약".to_string()));
        assert_eq!(parse_input("drop potion"), PlayerAction::Drop("potion".to_string()));
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
        assert_eq!(parse_input("말 안녕하세요"), PlayerAction::Say("안녕하세요".to_string()));
        assert_eq!(parse_input("say hello world"), PlayerAction::Say("hello world".to_string()));
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
    }

    #[test]
    fn parse_unknown() {
        assert_eq!(parse_input("dance"), PlayerAction::Unknown("dance".to_string()));
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!(parse_input("NORTH"), PlayerAction::Move(Direction::North));
        assert_eq!(parse_input("Look"), PlayerAction::Look);
        assert_eq!(parse_input("ATTACK Goblin"), PlayerAction::Attack("goblin".to_string()));
    }

    #[test]
    fn parse_whitespace_handling() {
        assert_eq!(parse_input("  north  "), PlayerAction::Move(Direction::North));
        assert_eq!(parse_input("  attack   goblin  "), PlayerAction::Attack("goblin".to_string()));
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
