//! WebSocket client for connecting to an RRP renderer.
//!
//! Requires the `client` feature. Provides a synchronous, caller-driven API
//! that performs the full 4-step handshake.
//!
//! ```no_run
//! use renderrelay::{RrpClient, JoinConfig};
//! use renderrelay::types::StreamFormat;
//!
//! let mut client = RrpClient::connect(
//!     "ws://192.168.1.50:8080/rrp",
//!     "my-viewer",
//!     &["0.1.0"],
//!     JoinConfig {
//!         format: StreamFormat::LlHls,
//!         extensions: Some(vec!["golf.frp".into()]),
//!         token: None,
//!     },
//! ).unwrap();
//!
//! println!("Stream URL: {}", client.stream_url());
//! ```

use std::fmt;
use std::net::TcpStream;

use tungstenite::protocol::WebSocket;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::Message;

use crate::error::RrpError;
use crate::message::RrpMessage;
use crate::types::{StreamFormat, StreamSelection};

/// Configuration for the `join` step of the RRP handshake.
#[derive(Debug, Clone)]
pub struct JoinConfig {
    /// The stream format to request.
    pub format: StreamFormat,
    /// Extensions the viewer supports.
    pub extensions: Option<Vec<String>>,
    /// Authentication token (if renderer requires auth).
    pub token: Option<String>,
}

/// A synchronous WebSocket client connected to an RRP renderer.
///
/// After [`connect`](Self::connect), the 4-step handshake is complete and the
/// stream URL is available.
pub struct RrpClient {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    version: String,
    stream_url: String,
    stream_format: StreamFormat,
    active_extensions: Vec<String>,
}

impl RrpClient {
    /// Connect to an RRP renderer and complete the 4-step handshake.
    ///
    /// 1. Send `start` with versions
    /// 2. Receive `init` with capabilities
    /// 3. Send `join` with format selection
    /// 4. Receive `stream_ready` with URL
    ///
    /// # Errors
    ///
    /// Returns an error if connection, handshake, or version negotiation fails.
    pub fn connect(
        url: &str,
        name: &str,
        versions: &[&str],
        join: JoinConfig,
    ) -> Result<Self, RrpError> {
        let (mut socket, _response) = tungstenite::connect(url)?;

        // Step 1: send start
        let start = RrpMessage::Start {
            version: versions.iter().map(|&s| s.to_owned()).collect(),
            name: Some(name.to_owned()),
        };
        socket.send(Message::text(start.to_json()?))?;

        // Step 2: receive init
        let init_msg = loop {
            match socket.read()? {
                Message::Text(text) => match RrpMessage::parse(&text)? {
                    msg @ RrpMessage::Init { .. } => break msg,
                    RrpMessage::Alert {
                        severity: crate::Severity::Critical,
                        message,
                    } => return Err(RrpError::Handshake(message)),
                    _ => {}
                },
                Message::Close(_) => return Err(RrpError::Closed),
                _ => {}
            }
        };

        // Step 3: send join
        let join_msg = RrpMessage::Join {
            stream: StreamSelection {
                selected: join.format,
            },
            extensions: join.extensions,
            token: join.token,
        };
        socket.send(Message::text(join_msg.to_json()?))?;

        // Step 4: receive stream_ready
        let (version, stream_url, stream_format, active_extensions) = loop {
            match socket.read()? {
                Message::Text(text) => match RrpMessage::parse(&text)? {
                    RrpMessage::StreamReady {
                        format,
                        url: stream_url,
                        extensions,
                    } => {
                        let ver = match &init_msg {
                            RrpMessage::Init { version, .. } => version.clone(),
                            _ => unreachable!(),
                        };
                        break (
                            ver,
                            stream_url,
                            format,
                            extensions.unwrap_or_default(),
                        );
                    }
                    RrpMessage::Alert {
                        severity: crate::Severity::Critical,
                        message,
                    } => return Err(RrpError::Handshake(message)),
                    _ => {}
                },
                Message::Close(_) => return Err(RrpError::Closed),
                _ => {}
            }
        };

        Ok(Self {
            socket,
            version,
            stream_url,
            stream_format,
            active_extensions,
        })
    }

    /// The RRP version negotiated during the handshake.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The video stream URL from the renderer.
    #[must_use]
    pub fn stream_url(&self) -> &str {
        &self.stream_url
    }

    /// The confirmed stream format.
    #[must_use]
    pub fn stream_format(&self) -> StreamFormat {
        self.stream_format
    }

    /// The active extensions confirmed by the renderer.
    #[must_use]
    pub fn active_extensions(&self) -> &[String] {
        &self.active_extensions
    }

    /// Set the underlying TCP stream to non-blocking mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the TCP stream mode cannot be set.
    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<(), RrpError> {
        match self.socket.get_ref() {
            MaybeTlsStream::Plain(tcp) => tcp
                .set_nonblocking(nonblocking)
                .map_err(|e| RrpError::WebSocket(Box::new(tungstenite::Error::Io(e)))),
            _ => Ok(()),
        }
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

    /// Send an [`RrpMessage`] to the renderer.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be serialized or sent.
    pub fn send(&mut self, msg: &RrpMessage) -> Result<(), RrpError> {
        let json = msg.to_json()?;
        self.socket.send(Message::text(json))?;
        Ok(())
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

impl fmt::Debug for RrpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RrpClient")
            .field("version", &self.version)
            .field("stream_url", &self.stream_url)
            .field("stream_format", &self.stream_format)
            .field("active_extensions", &self.active_extensions)
            .finish_non_exhaustive()
    }
}
