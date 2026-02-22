const IAC: u8 = 255;
const WILL: u8 = 251;
const WONT: u8 = 252;
const DO: u8 = 253;
const DONT: u8 = 254;
const SB: u8 = 250;
const SE: u8 = 240;

/// Strip Telnet IAC sequences from raw bytes.
pub fn strip_iac(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == IAC {
            if i + 1 >= bytes.len() {
                break; // incomplete IAC sequence
            }
            match bytes[i + 1] {
                WILL | WONT | DO | DONT => {
                    // 3-byte sequence: IAC + cmd + option
                    i += 3;
                }
                SB => {
                    // Subnegotiation: skip until IAC SE
                    i += 2;
                    while i + 1 < bytes.len() {
                        if bytes[i] == IAC && bytes[i + 1] == SE {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                IAC => {
                    // Escaped IAC (literal 255)
                    result.push(IAC);
                    i += 2;
                }
                _ => {
                    // Unknown 2-byte IAC command
                    i += 2;
                }
            }
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }

    result
}

const MAX_LINE_LEN: usize = 4096;

/// Buffered line reader for Telnet input.
pub struct LineBuffer {
    buf: Vec<u8>,
}

impl LineBuffer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Feed raw data into the buffer. Returns any complete lines.
    pub fn feed(&mut self, data: &[u8]) -> Vec<String> {
        let cleaned = strip_iac(data);
        let mut lines = Vec::new();

        for &byte in &cleaned {
            if byte == b'\n' {
                let line = self.take_line();
                lines.push(line);
            } else if byte == b'\r' {
                // Ignore CR, we split on LF
            } else {
                if self.buf.len() < MAX_LINE_LEN {
                    self.buf.push(byte);
                }
                // Silently drop bytes beyond MAX_LINE_LEN
            }
        }

        lines
    }

    fn take_line(&mut self) -> String {
        let bytes = std::mem::take(&mut self.buf);
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl Default for LineBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_iac_basic() {
        let data = b"hello";
        assert_eq!(strip_iac(data), b"hello");
    }

    #[test]
    fn strip_iac_will_command() {
        // IAC WILL ECHO
        let data = [b'h', b'i', IAC, WILL, 1, b'!'];
        assert_eq!(strip_iac(&data), b"hi!");
    }

    #[test]
    fn strip_iac_do_command() {
        let data = [IAC, DO, 3, b'a', b'b'];
        assert_eq!(strip_iac(&data), b"ab");
    }

    #[test]
    fn strip_iac_subnegotiation() {
        let data = [b'x', IAC, SB, 24, 0, IAC, SE, b'y'];
        assert_eq!(strip_iac(&data), b"xy");
    }

    #[test]
    fn strip_iac_escaped_iac() {
        let data = [IAC, IAC, b'z'];
        assert_eq!(strip_iac(&data), vec![IAC, b'z']);
    }

    #[test]
    fn strip_iac_multiple_sequences() {
        let data = [IAC, WILL, 1, IAC, WONT, 3, IAC, DONT, 24, b'o', b'k'];
        assert_eq!(strip_iac(&data), b"ok");
    }

    #[test]
    fn line_buffer_basic() {
        let mut lb = LineBuffer::new();
        let lines = lb.feed(b"hello\n");
        assert_eq!(lines, vec!["hello"]);
    }

    #[test]
    fn line_buffer_multiple_lines() {
        let mut lb = LineBuffer::new();
        let lines = lb.feed(b"line1\nline2\n");
        assert_eq!(lines, vec!["line1", "line2"]);
    }

    #[test]
    fn line_buffer_partial() {
        let mut lb = LineBuffer::new();
        let lines1 = lb.feed(b"hel");
        assert!(lines1.is_empty());
        let lines2 = lb.feed(b"lo\n");
        assert_eq!(lines2, vec!["hello"]);
    }

    #[test]
    fn line_buffer_crlf() {
        let mut lb = LineBuffer::new();
        let lines = lb.feed(b"hello\r\n");
        assert_eq!(lines, vec!["hello"]);
    }

    #[test]
    fn line_buffer_overflow() {
        let mut lb = LineBuffer::new();
        let long_data: Vec<u8> = vec![b'x'; 5000];
        lb.feed(&long_data);
        let lines = lb.feed(b"\n");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), MAX_LINE_LEN);
    }

    #[test]
    fn line_buffer_strips_iac_in_feed() {
        let mut lb = LineBuffer::new();
        let data = [b'h', IAC, WILL, 1, b'i', b'\n'];
        let lines = lb.feed(&data);
        assert_eq!(lines, vec!["hi"]);
    }
}
