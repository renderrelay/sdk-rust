//! WebSocket server for accepting RRP viewer connections.
//!
//! Requires the `server` feature. Provides a synchronous, caller-driven API
//! that performs the inverse 4-step handshake.
//!
//! ```no_run
//! use renderrelay::{RrpListener, RendererConfig};
//! use renderrelay::types::*;
//!
//! let config = RendererConfig {
//!     name: Some("My Golf Sim".into()),
//!     supported_versions: vec!["0.1.0".into()],
//!     auth: AuthMode::None,
//!     auth_endpoint: None,
//!     stream: StreamCaps { supported: vec![StreamFormat::LlHls] },
//!     input: InputCaps { keys: keys::REQUIRED.iter().map(|&s| s.into()).collect() },
//!     extensions: Some(vec!["golf.frp".into()]),
//! };
//!
//! let listener = RrpListener::bind("0.0.0.0:8080", config).unwrap();
//! let mut conn = listener.accept(|format, extensions| {
//!     // Return stream URL for the requested format
//!     Ok(("http://192.168.1.50:8080/stream.m3u8".into(), extensions))
//! }).unwrap();
//! ```

use std::fmt;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

use tungstenite::protocol::WebSocket;
use tungstenite::Message;

use crate::error::RrpError;
use crate::message::RrpMessage;
use crate::types::{AuthMode, InputCaps, Severity, StreamCaps, StreamFormat};

/// Configuration for the renderer side of an RRP connection.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    /// Human-readable renderer name.
    pub name: Option<String>,
    /// RRP versions this renderer supports.
    pub supported_versions: Vec<String>,
    /// Authentication mode.
    pub auth: AuthMode,
    /// OAuth 2.0 Device Authorization endpoint (required when `auth` is `Required`).
    pub auth_endpoint: Option<String>,
    /// Supported stream formats.
    pub stream: StreamCaps,
    /// Accepted input keys.
    pub input: InputCaps,
    /// Supported extensions.
    pub extensions: Option<Vec<String>>,
}

/// Listens for incoming RRP viewer connections.
pub struct RrpListener {
    listener: TcpListener,
    config: RendererConfig,
}

impl RrpListener {
    /// Bind to the given address and listen for RRP connections.
    ///
    /// # Errors
    ///
    /// Returns an error if the TCP listener cannot bind.
    pub fn bind(addr: impl ToSocketAddrs, config: RendererConfig) -> Result<Self, RrpError> {
        let listener = TcpListener::bind(addr)
            .map_err(|e| RrpError::WebSocket(Box::new(tungstenite::Error::Io(e))))?;
        Ok(Self { listener, config })
    }

    /// Accept a single incoming connection and perform the RRP handshake.
    ///
    /// The `stream_provider` callback is called with the viewer's selected format
    /// and requested extensions. It must return `(stream_url, active_extensions)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection or handshake fails.
    pub fn accept<F>(&self, stream_provider: F) -> Result<RrpConnection, RrpError>
    where
        F: FnOnce(StreamFormat, Vec<String>) -> Result<(String, Vec<String>), RrpError>,
    {
        let (tcp_stream, _addr) = self
            .listener
            .accept()
            .map_err(|e| RrpError::WebSocket(Box::new(tungstenite::Error::Io(e))))?;
        let mut socket = tungstenite::accept(tcp_stream).map_err(|e| match e {
            tungstenite::HandshakeError::Failure(e) => RrpError::WebSocket(Box::new(e)),
            tungstenite::HandshakeError::Interrupted(_) => {
                RrpError::Handshake("WebSocket handshake interrupted".into())
            }
        })?;

        // Step 1: receive start
        let (client_versions, client_name) = loop {
            match socket.read()? {
                Message::Text(text) => {
                    if let Ok(RrpMessage::Start { version, name }) = RrpMessage::parse(&text) {
                        break (version, name);
                    }
                }
                Message::Close(_) => return Err(RrpError::Closed),
                _ => {}
            }
        };

        // Select version
        let selected_version = select_version(&self.config.supported_versions, &client_versions);
        let Some(version) = selected_version else {
            let alert = RrpMessage::Alert {
                severity: Severity::Critical,
                message: "No compatible RRP version".into(),
            };
            socket.send(Message::text(alert.to_json()?))?;
            socket.close(None)?;
            return Err(RrpError::Handshake("No compatible RRP version".into()));
        };

        // Step 2: send init
        let init = RrpMessage::Init {
            version: version.clone(),
            name: self.config.name.clone(),
            auth: self.config.auth,
            auth_endpoint: self.config.auth_endpoint.clone(),
            stream: self.config.stream.clone(),
            input: self.config.input.clone(),
            extensions: self.config.extensions.clone(),
        };
        socket.send(Message::text(init.to_json()?))?;

        // Step 3: receive join
        let (selected_format, viewer_extensions, _token) = loop {
            match socket.read()? {
                Message::Text(text) => {
                    if let Ok(RrpMessage::Join {
                        stream,
                        extensions,
                        token,
                    }) = RrpMessage::parse(&text)
                    {
                        break (stream.selected, extensions.unwrap_or_default(), token);
                    }
                }
                Message::Close(_) => return Err(RrpError::Closed),
                _ => {}
            }
        };

        // Compute active extensions (intersection)
        let server_exts = self.config.extensions.as_deref().unwrap_or_default();
        let requested_exts: Vec<String> = viewer_extensions
            .into_iter()
            .filter(|e| server_exts.contains(e))
            .collect();

        // Call stream provider
        let (stream_url, active_extensions) =
            stream_provider(selected_format, requested_exts)?;

        // Step 4: send stream_ready
        let stream_ready = RrpMessage::StreamReady {
            format: selected_format,
            url: stream_url.clone(),
            extensions: if active_extensions.is_empty() {
                None
            } else {
                Some(active_extensions.clone())
            },
        };
        socket.send(Message::text(stream_ready.to_json()?))?;

        Ok(RrpConnection {
            socket,
            version,
            client_name,
            stream_url,
            stream_format: selected_format,
            active_extensions,
        })
    }
}

impl fmt::Debug for RrpListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RrpListener")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// An established RRP connection from a viewer.
pub struct RrpConnection {
    socket: WebSocket<TcpStream>,
    version: String,
    client_name: Option<String>,
    stream_url: String,
    stream_format: StreamFormat,
    active_extensions: Vec<String>,
}

impl RrpConnection {
    /// The RRP version negotiated during the handshake.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The viewer name from the `start` message, if provided.
    #[must_use]
    pub fn client_name(&self) -> Option<&str> {
        self.client_name.as_deref()
    }

    /// The stream URL provided to the viewer.
    #[must_use]
    pub fn stream_url(&self) -> &str {
        &self.stream_url
    }

    /// The confirmed stream format.
    #[must_use]
    pub fn stream_format(&self) -> StreamFormat {
        self.stream_format
    }

    /// The active extensions confirmed during the handshake.
    #[must_use]
    pub fn active_extensions(&self) -> &[String] {
        &self.active_extensions
    }

    /// Block until the next [`RrpMessage`] arrives.
    ///
    /// # Errors
    ///
    /// Returns an error on connection failure or invalid JSON.
    pub fn recv(&mut self) -> Result<RrpMessage, RrpError> {
        loop {
            match self.socket.read()? {
                Message::Text(text) => return RrpMessage::parse(&text),
                Message::Close(_) => return Err(RrpError::Closed),
                _ => {}
            }
        }
    }

    /// Poll for the next message without blocking.
    ///
    /// # Errors
    ///
    /// Returns an error on connection failure or invalid JSON.
    pub fn try_recv(&mut self) -> Result<Option<RrpMessage>, RrpError> {
        loop {
            match self.socket.read() {
                Ok(Message::Text(text)) => return Ok(Some(RrpMessage::parse(&text)?)),
                Ok(Message::Close(_)) => return Err(RrpError::Closed),
                Ok(_) => {}
                Err(tungstenite::Error::Io(ref e))
                    if e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    return Ok(None);
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Send an [`RrpMessage`] to the viewer.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be serialized or sent.
    pub fn send(&mut self, msg: &RrpMessage) -> Result<(), RrpError> {
        let json = msg.to_json()?;
        self.socket.send(Message::text(json))?;
        Ok(())
    }

    /// Set the underlying TCP stream to non-blocking mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the TCP stream mode cannot be set.
    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<(), RrpError> {
        self.socket
            .get_ref()
            .set_nonblocking(nonblocking)
            .map_err(|e| RrpError::WebSocket(Box::new(tungstenite::Error::Io(e))))
    }

    /// Cleanly close the WebSocket connection.
    ///
    /// # Errors
    ///
    /// Returns an error if the close handshake fails.
    pub fn close(mut self) -> Result<(), RrpError> {
        self.socket.close(None)?;
        loop {
            match self.socket.read() {
                Ok(Message::Close(_)) | Err(tungstenite::Error::ConnectionClosed) => {
                    return Ok(());
                }
                Err(tungstenite::Error::AlreadyClosed) => return Ok(()),
                Err(e) => return Err(e.into()),
                _ => {}
            }
        }
    }
}

impl fmt::Debug for RrpConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RrpConnection")
            .field("version", &self.version)
            .field("client_name", &self.client_name)
            .field("stream_format", &self.stream_format)
            .finish_non_exhaustive()
    }
}

/// Select the highest version present in both lists.
fn select_version(server: &[String], client: &[String]) -> Option<String> {
    for cv in client {
        if server.contains(cv) {
            return Some(cv.clone());
        }
    }
    None
}
