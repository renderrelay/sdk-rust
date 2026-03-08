//! RRP domain types: stream formats, key states, capabilities.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Video stream format identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamFormat {
    #[serde(rename = "ll-hls")]
    LlHls,
    #[serde(rename = "hls")]
    Hls,
    #[serde(rename = "dash")]
    Dash,
    #[serde(rename = "webrtc")]
    WebRtc,
    #[serde(rename = "rtsp")]
    Rtsp,
}

impl fmt::Display for StreamFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LlHls => write!(f, "ll-hls"),
            Self::Hls => write!(f, "hls"),
            Self::Dash => write!(f, "dash"),
            Self::WebRtc => write!(f, "webrtc"),
            Self::Rtsp => write!(f, "rtsp"),
        }
    }
}

/// Key press state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyState {
    Down,
    Up,
    Repeat,
}

/// Alert severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warn,
    Error,
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Authentication mode advertised in `init`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    None,
    Required,
}

/// Stream capabilities advertised in `init`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamCaps {
    pub supported: Vec<StreamFormat>,
}

/// Input capabilities advertised in `init`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputCaps {
    pub keys: Vec<String>,
}

/// Stream format selection sent in `join`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamSelection {
    pub selected: StreamFormat,
}

/// Standard key name constants.
pub mod keys {
    // Required keys
    pub const UP: &str = "up";
    pub const DOWN: &str = "down";
    pub const LEFT: &str = "left";
    pub const RIGHT: &str = "right";
    pub const OK: &str = "ok";
    pub const BACK: &str = "back";
    pub const PLAYPAUSE: &str = "playpause";

    // Optional keys
    pub const PLAY: &str = "play";
    pub const PAUSE: &str = "pause";
    pub const REWIND: &str = "rewind";
    pub const FASTFORWARD: &str = "fastforward";
    pub const OPTIONS: &str = "options";
    pub const HOME: &str = "home";

    /// All required keys that every compliant viewer must support.
    pub const REQUIRED: &[&str] = &[UP, DOWN, LEFT, RIGHT, OK, BACK, PLAYPAUSE];
}
