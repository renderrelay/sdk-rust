//! Render Relay Protocol (RRP) — server-rendered streaming to TVs and displays.
//!
//! This crate provides message schemas and optional WebSocket transport for the
//! [Render Relay Protocol](https://github.com/renderrelay/spec).
//!
//! # Features
//!
//! - **`viewer`** — (reserved for future viewer-side helpers)
//! - **`renderer`** — (reserved for future renderer-side helpers)
//! - **`client`** — [`RrpClient`] WebSocket client (connects to a renderer)
//! - **`server`** — [`RrpListener`] / [`RrpConnection`] WebSocket server (accepts viewers)

pub mod error;
pub mod message;
pub mod types;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

pub use error::RrpError;
pub use message::RrpMessage;
pub use types::{
    AuthMode, InputCaps, KeyState, Severity, StreamCaps, StreamFormat, StreamSelection,
};

#[cfg(feature = "client")]
pub use client::{JoinConfig, RrpClient};

#[cfg(feature = "server")]
pub use server::{RendererConfig, RrpConnection, RrpListener};

/// The RRP spec version this crate implements.
pub const SPEC_VERSION: &str = "0.1.0";

/// Default WebSocket path for RRP connections.
pub const DEFAULT_PATH: &str = "/rrp";
