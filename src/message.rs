//! RRP message types.
//!
//! All RRP messages share `"type"` at the top level, making them a clean
//! internally tagged enum.

use serde::{Deserialize, Serialize};

use crate::error::RrpError;
use crate::types::{AuthMode, InputCaps, KeyState, Severity, StreamCaps, StreamFormat, StreamSelection};

/// An RRP message, tagged by `"type"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RrpMessage {
    /// Viewer → Renderer: version negotiation.
    Start {
        version: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// Renderer → Viewer: capabilities and version confirmation.
    Init {
        version: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        auth: AuthMode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        auth_endpoint: Option<String>,
        stream: StreamCaps,
        input: InputCaps,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extensions: Option<Vec<String>>,
    },
    /// Viewer → Renderer: format selection and auth.
    Join {
        stream: StreamSelection,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extensions: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
    },
    /// Renderer → Viewer: stream URL and confirmed extensions.
    StreamReady {
        format: StreamFormat,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extensions: Option<Vec<String>>,
    },
    /// Input event (viewer → renderer).
    Key {
        key: String,
        state: KeyState,
    },
    /// Extension event (bidirectional).
    Ext {
        ext: String,
        data: serde_json::Value,
    },
    /// Stream URL update (renderer → viewer).
    StreamUpdate {
        url: String,
    },
    /// Notification (renderer → viewer).
    Notify {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration: Option<u64>,
    },
    /// Alert (either direction).
    Alert {
        severity: Severity,
        message: String,
    },
    /// Unknown message type — per spec, must be silently ignored.
    #[serde(other)]
    Unknown,
}

impl RrpMessage {
    /// Parse a JSON string into an `RrpMessage`.
    ///
    /// # Errors
    ///
    /// Returns `RrpError::Json` if the JSON is malformed or unrecognized.
    pub fn parse(json: &str) -> Result<Self, RrpError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Serialize this message to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns `RrpError::Json` if serialization fails.
    pub fn to_json(&self) -> Result<String, RrpError> {
        Ok(serde_json::to_string(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_start() {
        let json = r#"{"type":"start","version":["0.1.0"],"name":"roku"}"#;
        let msg = RrpMessage::parse(json).unwrap();
        match msg {
            RrpMessage::Start { version, name } => {
                assert_eq!(version, vec!["0.1.0"]);
                assert_eq!(name.as_deref(), Some("roku"));
            }
            _ => panic!("expected Start"),
        }
    }

    #[test]
    fn parse_init() {
        let json = r#"{
            "type": "init",
            "version": "0.1.0",
            "name": "My Golf Sim",
            "auth": "none",
            "stream": {
                "supported": ["ll-hls", "hls", "dash", "webrtc", "rtsp"]
            },
            "input": {
                "keys": ["up", "down", "left", "right", "ok", "back", "playpause"]
            },
            "extensions": ["golf.frp"]
        }"#;
        let msg = RrpMessage::parse(json).unwrap();
        match msg {
            RrpMessage::Init {
                version,
                auth,
                stream,
                extensions,
                ..
            } => {
                assert_eq!(version, "0.1.0");
                assert_eq!(auth, AuthMode::None);
                assert_eq!(stream.supported.len(), 5);
                assert_eq!(stream.supported[0], StreamFormat::LlHls);
                assert_eq!(extensions.as_deref(), Some(&["golf.frp".to_owned()][..]));
            }
            _ => panic!("expected Init"),
        }
    }

    #[test]
    fn parse_join() {
        let json = r#"{
            "type": "join",
            "stream": {"selected": "ll-hls"},
            "extensions": ["golf.frp"]
        }"#;
        let msg = RrpMessage::parse(json).unwrap();
        match msg {
            RrpMessage::Join {
                stream,
                extensions,
                token,
            } => {
                assert_eq!(stream.selected, StreamFormat::LlHls);
                assert!(extensions.is_some());
                assert!(token.is_none());
            }
            _ => panic!("expected Join"),
        }
    }

    #[test]
    fn parse_join_with_token() {
        let json = r#"{
            "type": "join",
            "stream": {"selected": "ll-hls"},
            "extensions": ["golf.frp"],
            "token": "rrp_tok_xxxxxxxx"
        }"#;
        let msg = RrpMessage::parse(json).unwrap();
        match msg {
            RrpMessage::Join { token, .. } => {
                assert_eq!(token.as_deref(), Some("rrp_tok_xxxxxxxx"));
            }
            _ => panic!("expected Join"),
        }
    }

    #[test]
    fn parse_stream_ready() {
        let json = r#"{
            "type": "stream_ready",
            "format": "ll-hls",
            "url": "http://192.168.1.50:8080/stream.m3u8",
            "extensions": ["golf.frp"]
        }"#;
        let msg = RrpMessage::parse(json).unwrap();
        match msg {
            RrpMessage::StreamReady {
                format,
                url,
                extensions,
            } => {
                assert_eq!(format, StreamFormat::LlHls);
                assert_eq!(url, "http://192.168.1.50:8080/stream.m3u8");
                assert!(extensions.is_some());
            }
            _ => panic!("expected StreamReady"),
        }
    }

    #[test]
    fn parse_key() {
        let json = r#"{"type":"key","key":"left","state":"down"}"#;
        let msg = RrpMessage::parse(json).unwrap();
        match msg {
            RrpMessage::Key { key, state } => {
                assert_eq!(key, "left");
                assert_eq!(state, KeyState::Down);
            }
            _ => panic!("expected Key"),
        }
    }

    #[test]
    fn parse_ext() {
        let json = r#"{
            "type": "ext",
            "ext": "golf.frp",
            "data": {
                "device": "EagleOne-X4K2",
                "kind": "ball_flight",
                "key": {"shot_id": "abc", "shot_number": 1},
                "ball": {"launch_speed": "67.2mps"}
            }
        }"#;
        let msg = RrpMessage::parse(json).unwrap();
        match msg {
            RrpMessage::Ext { ext, data } => {
                assert_eq!(ext, "golf.frp");
                assert_eq!(data["device"], "EagleOne-X4K2");
                assert_eq!(data["kind"], "ball_flight");
            }
            _ => panic!("expected Ext"),
        }
    }

    #[test]
    fn parse_stream_update() {
        let json = r#"{"type":"stream_update","url":"http://192.168.1.50:8080/stream2.m3u8"}"#;
        let msg = RrpMessage::parse(json).unwrap();
        match msg {
            RrpMessage::StreamUpdate { url } => {
                assert_eq!(url, "http://192.168.1.50:8080/stream2.m3u8");
            }
            _ => panic!("expected StreamUpdate"),
        }
    }

    #[test]
    fn parse_notify() {
        let json = r#"{"type":"notify","message":"Club changed: 7 Iron","duration":2000}"#;
        let msg = RrpMessage::parse(json).unwrap();
        match msg {
            RrpMessage::Notify { message, duration } => {
                assert_eq!(message, "Club changed: 7 Iron");
                assert_eq!(duration, Some(2000));
            }
            _ => panic!("expected Notify"),
        }
    }

    #[test]
    fn parse_alert() {
        let json = r#"{"type":"alert","severity":"critical","message":"Authentication required"}"#;
        let msg = RrpMessage::parse(json).unwrap();
        match msg {
            RrpMessage::Alert { severity, message } => {
                assert_eq!(severity, Severity::Critical);
                assert_eq!(message, "Authentication required");
            }
            _ => panic!("expected Alert"),
        }
    }

    #[test]
    fn roundtrip_all_variants() {
        let messages = vec![
            RrpMessage::Start {
                version: vec!["0.1.0".into()],
                name: Some("test".into()),
            },
            RrpMessage::Init {
                version: "0.1.0".into(),
                name: Some("Sim".into()),
                auth: AuthMode::None,
                auth_endpoint: None,
                stream: StreamCaps {
                    supported: vec![StreamFormat::LlHls],
                },
                input: InputCaps {
                    keys: vec!["ok".into()],
                },
                extensions: Some(vec!["golf.frp".into()]),
            },
            RrpMessage::Join {
                stream: StreamSelection {
                    selected: StreamFormat::LlHls,
                },
                extensions: None,
                token: None,
            },
            RrpMessage::StreamReady {
                format: StreamFormat::Hls,
                url: "http://example.com/stream.m3u8".into(),
                extensions: None,
            },
            RrpMessage::Key {
                key: "up".into(),
                state: KeyState::Down,
            },
            RrpMessage::Ext {
                ext: "golf.frp".into(),
                data: serde_json::json!({"kind": "start"}),
            },
            RrpMessage::StreamUpdate {
                url: "http://example.com/stream2.m3u8".into(),
            },
            RrpMessage::Notify {
                message: "Hello".into(),
                duration: Some(1000),
            },
            RrpMessage::Alert {
                severity: Severity::Warn,
                message: "test".into(),
            },
        ];

        for msg in messages {
            let json = msg.to_json().unwrap();
            let back = RrpMessage::parse(&json).unwrap();
            assert_eq!(msg, back, "roundtrip failed for {json}");
        }
    }

    #[test]
    fn stream_format_kebab_case() {
        let json = serde_json::to_string(&StreamFormat::LlHls).unwrap();
        assert_eq!(json, "\"ll-hls\"");
        let back: StreamFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(back, StreamFormat::LlHls);
    }

    #[test]
    fn unknown_type_parses_as_unknown() {
        let json = r#"{"type":"future_message","data":"something"}"#;
        let msg = RrpMessage::parse(json).unwrap();
        assert_eq!(msg, RrpMessage::Unknown);
    }
}
