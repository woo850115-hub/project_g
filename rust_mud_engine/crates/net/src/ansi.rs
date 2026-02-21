/// ANSI escape code constants for MUD text coloring.

// Reset
pub const RESET: &str = "\x1b[0m";

// Styles
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const UNDERLINE: &str = "\x1b[4m";

// Foreground colors
pub const BLACK: &str = "\x1b[30m";
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const BLUE: &str = "\x1b[34m";
pub const MAGENTA: &str = "\x1b[35m";
pub const CYAN: &str = "\x1b[36m";
pub const WHITE: &str = "\x1b[37m";

// Bright foreground
pub const BRIGHT_RED: &str = "\x1b[91m";
pub const BRIGHT_GREEN: &str = "\x1b[92m";
pub const BRIGHT_YELLOW: &str = "\x1b[93m";
pub const BRIGHT_BLUE: &str = "\x1b[94m";
pub const BRIGHT_MAGENTA: &str = "\x1b[95m";
pub const BRIGHT_CYAN: &str = "\x1b[96m";
pub const BRIGHT_WHITE: &str = "\x1b[97m";

/// Strip all ANSI escape sequences from a string.
pub fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // Skip ESC [ ... until a letter (0x40-0x7E)
            i += 2;
            while i < bytes.len() {
                let b = bytes[i];
                i += 1;
                if (0x40..=0x7E).contains(&b) {
                    break;
                }
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    result
}

/// Wrap text with a color and auto-reset.
pub fn colorize(color: &str, text: &str) -> String {
    format!("{}{}{}", color, text, RESET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_no_codes() {
        assert_eq!(strip_ansi("hello world"), "hello world");
    }

    #[test]
    fn strip_ansi_single_code() {
        let input = format!("{}hello{}", RED, RESET);
        assert_eq!(strip_ansi(&input), "hello");
    }

    #[test]
    fn strip_ansi_multiple_codes() {
        let input = format!("{}bold{} {}red{}", BOLD, RESET, RED, RESET);
        assert_eq!(strip_ansi(&input), "bold red");
    }

    #[test]
    fn strip_ansi_nested() {
        let input = format!("{}{}bright red{}", BOLD, RED, RESET);
        assert_eq!(strip_ansi(&input), "bright red");
    }

    #[test]
    fn strip_ansi_empty() {
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn colorize_wraps_text() {
        let colored = colorize(RED, "danger");
        assert!(colored.starts_with(RED));
        assert!(colored.ends_with(RESET));
        assert_eq!(strip_ansi(&colored), "danger");
    }
}
