/// GMCP (Generic MUD Communication Protocol) package support.
/// GMCP sends structured data over Telnet subnegotiation.

use serde::Serialize;

/// Telnet option code for GMCP
pub const GMCP_OPTION: u8 = 201;

/// GMCP Char.Vitals package — player vital statistics.
#[derive(Debug, Clone, Serialize)]
pub struct CharVitals {
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
}

/// GMCP Room.Info package — current room information.
#[derive(Debug, Clone, Serialize)]
pub struct RoomInfo {
    pub name: String,
    pub exits: Vec<String>,
}

/// Serialize a GMCP package to the wire format: "Package.Name json_data"
pub fn serialize_gmcp(package: &str, data: &impl Serialize) -> String {
    let json = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    format!("{} {}", package, json)
}

/// Build Telnet subnegotiation bytes for a GMCP message.
/// Format: IAC SB GMCP <payload> IAC SE
pub fn gmcp_subneg(payload: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(payload.len() + 5);
    bytes.push(255); // IAC
    bytes.push(250); // SB
    bytes.push(GMCP_OPTION);
    bytes.extend_from_slice(payload.as_bytes());
    bytes.push(255); // IAC
    bytes.push(240); // SE
    bytes
}

/// Build Telnet negotiation to request GMCP support.
/// Sends IAC WILL GMCP to indicate server supports GMCP.
pub fn gmcp_will() -> [u8; 3] {
    [255, 251, GMCP_OPTION] // IAC WILL GMCP
}

/// Check if a Telnet DO response matches GMCP.
pub fn is_gmcp_do(data: &[u8]) -> bool {
    data.len() >= 3 && data[0] == 255 && data[1] == 253 && data[2] == GMCP_OPTION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_char_vitals() {
        let vitals = CharVitals {
            hp: 85,
            max_hp: 100,
            atk: 10,
            def: 3,
        };
        let msg = serialize_gmcp("Char.Vitals", &vitals);
        assert!(msg.starts_with("Char.Vitals "));
        assert!(msg.contains("\"hp\":85"));
        assert!(msg.contains("\"max_hp\":100"));
    }

    #[test]
    fn serialize_room_info() {
        let room = RoomInfo {
            name: "Starting Room".to_string(),
            exits: vec!["north".to_string(), "east".to_string()],
        };
        let msg = serialize_gmcp("Room.Info", &room);
        assert!(msg.starts_with("Room.Info "));
        assert!(msg.contains("\"name\":\"Starting Room\""));
        assert!(msg.contains("\"exits\":[\"north\",\"east\"]"));
    }

    #[test]
    fn gmcp_subneg_format() {
        let payload = "Char.Vitals {\"hp\":100}";
        let bytes = gmcp_subneg(payload);
        assert_eq!(bytes[0], 255); // IAC
        assert_eq!(bytes[1], 250); // SB
        assert_eq!(bytes[2], GMCP_OPTION);
        assert_eq!(bytes[bytes.len() - 2], 255); // IAC
        assert_eq!(bytes[bytes.len() - 1], 240); // SE
        let inner = &bytes[3..bytes.len() - 2];
        assert_eq!(std::str::from_utf8(inner).unwrap(), payload);
    }

    #[test]
    fn gmcp_will_format() {
        let will = gmcp_will();
        assert_eq!(will, [255, 251, GMCP_OPTION]);
    }

    #[test]
    fn is_gmcp_do_check() {
        assert!(is_gmcp_do(&[255, 253, GMCP_OPTION]));
        assert!(!is_gmcp_do(&[255, 253, 1])); // Not GMCP
        assert!(!is_gmcp_do(&[255, 251, GMCP_OPTION])); // WILL, not DO
        assert!(!is_gmcp_do(&[255])); // Too short
    }
}
