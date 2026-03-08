# renderrelay

Rust SDK for the [Render Relay Protocol (RRP)](https://github.com/renderrelay/spec) — server-rendered streaming to TVs and displays.

[![CI](https://github.com/renderrelay/sdk-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/renderrelay/sdk-rust/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/renderrelay.svg)](https://crates.io/crates/renderrelay)

## What's in the box

- **Message types** — strongly typed Rust representations of every RRP message (handshake, input, extensions, alerts)
- **4-step handshake** — `start` → `init` → `join` → `stream_ready` with version negotiation, capability exchange, and extension selection
- **Input events** — standard remote keys (`up`, `down`, `ok`, `back`, `playpause`, etc.) with key state tracking
- **Extension system** — dot-namespaced extension events (`golf.frp`, `birdielabs.golf.frp`) for domain-specific protocols
- **WebSocket transport** — `RrpClient` and `RrpListener`/`RrpConnection` handle the full handshake and provide synchronous send/recv

## Features

| Feature | Description |
|---|---|
| *(default)* | Message types, parsing, capability types |
| `viewer` | Reserved for future viewer-side helpers |
| `renderer` | Reserved for future renderer-side helpers |
| `client` | `RrpClient` — WebSocket client (connects to a renderer) |
| `server` | `RrpListener` / `RrpConnection` — WebSocket server (accepts viewers) |

The `client` and `server` features add a dependency on `tungstenite`.

## Usage

### Renderer (serving streams)

```rust
use renderrelay::{RrpListener, RrpMessage, RendererConfig, StreamCaps, StreamFormat, AuthMode};

let config = RendererConfig {
    name: "My Renderer".into(),
    versions: vec!["0.1.0".into()],
    auth: AuthMode::None,
    stream: StreamCaps {
        formats: vec![StreamFormat::LlHls, StreamFormat::Hls],
    },
    input_keys: vec!["up".into(), "down".into(), "left".into(), "right".into(), "ok".into(), "back".into()],
    extensions: vec!["golf.frp".into()],
};

let listener = RrpListener::bind("0.0.0.0:8080", config)?;

// accept() calls your closure to provide the stream URL once format is selected
let mut conn = listener.accept(|format, extensions| {
    Ok(("http://192.168.1.10:8080/stream/master.m3u8".into(), extensions))
})?;

loop {
    match conn.recv()? {
        RrpMessage::Key { key, state, .. } => println!("key: {} {:?}", key, state),
        RrpMessage::Ext { extension, data, .. } => println!("ext: {} {}", extension, data),
        _ => {}
    }
}
```

```toml
[dependencies]
renderrelay = { version = "0.1", features = ["server"] }
```

### Viewer (connecting to a renderer)

```rust
use renderrelay::{RrpClient, JoinConfig, StreamFormat, RrpMessage};

let join = JoinConfig {
    format: StreamFormat::LlHls,
    extensions: vec!["golf.frp".into()],
    token: None,
};

let mut client = RrpClient::connect("ws://192.168.1.10:8080/rrp", "My Viewer", join)?;

println!("Stream URL: {}", client.stream_url());
println!("Active extensions: {:?}", client.active_extensions());

// Forward input
client.send(&RrpMessage::key("ok", renderrelay::KeyState::Down))?;
```

```toml
[dependencies]
renderrelay = { version = "0.1", features = ["client"] }
```

### Messages only (no WebSocket)

```rust
use renderrelay::RrpMessage;

let msg = RrpMessage::parse(r#"{"type":"key","key":"ok","state":"down"}"#)?;
let json = msg.to_json()?;
```

```toml
[dependencies]
renderrelay = "0.0.1"
```

## Protocol

RRP enables server-rendered applications on TVs. The renderer renders everything and streams it as video (LL-HLS recommended). The viewer is a dumb terminal — it plays the stream and forwards remote control input back over WebSocket.

```
Renderer  ──▶  Video Stream (LL-HLS)  ──▶  TV
TV Remote ──▶  Key Events (WebSocket)  ──▶  Renderer
```

The handshake negotiates version, video format, authentication, and extensions. After `stream_ready`, the viewer starts playing the stream URL and the renderer listens for input.

See the [full spec](https://github.com/renderrelay/spec) for details. The [`golf.frp` extension](https://github.com/renderrelay/spec/blob/main/extensions/GOLF.md) tunnels [Flight Relay Protocol](https://github.com/flightrelay/spec) events for launch monitor integration.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT), at your option.
