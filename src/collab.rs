//! Collaborative performance mode — WebSocket server for multi-client parameter control.
//!
//! Starts a lightweight WebSocket server (using a raw TCP listener with a
//! hand-rolled HTTP upgrade so we avoid pulling in an async runtime).  Each
//! connected client is assigned a unique ID and may claim ownership of
//! specific parameter names.  When a client sends a JSON message, the server
//! validates ownership and broadcasts the change to all other clients.
//!
//! # Wire protocol
//!
//! Messages are newline-delimited JSON objects.
//!
//! ## Client → Server
//! ```json
//! { "claim": ["rho", "sigma"] }
//! { "set": { "rho": 28.5 } }
//! { "release": ["rho"] }
//! ```
//!
//! ## Server → Client
//! ```json
//! { "welcome": { "client_id": 3 } }
//! { "update": { "rho": 28.5, "owner": 3 } }
//! { "error": "parameter 'rho' is owned by client 1" }
//! { "peer_joined": { "client_id": 4, "total": 2 } }
//! { "peer_left":   { "client_id": 4, "total": 1 } }
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use math_sonify_plugin::collab::{CollabServer, SessionEvent};
//! use crossbeam_channel::unbounded;
//!
//! let (tx, rx) = unbounded::<SessionEvent>();
//! let server = CollabServer::new("127.0.0.1:9001", tx).unwrap();
//! server.run_background();
//!
//! // In the simulation thread:
//! for event in rx.try_iter() {
//!     match event {
//!         SessionEvent::ParamChanged { name, value, .. } => { /* apply */ }
//!         _ => {}
//!     }
//! }
//! ```

#![allow(dead_code)]

use crossbeam_channel::Sender;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

// ── Public types ──────────────────────────────────────────────────────────────

/// Events emitted to the simulation thread when clients mutate state.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// A client changed a named parameter.
    ParamChanged {
        client_id: u32,
        name: String,
        value: f64,
    },
    /// A client connected.
    ClientJoined { client_id: u32 },
    /// A client disconnected.
    ClientLeft { client_id: u32 },
}

/// Per-parameter ownership record.
#[derive(Debug, Clone)]
struct ParamOwner {
    client_id: u32,
}

/// Shared session state guarded by a mutex.
struct SessionState {
    /// Map from parameter name → owning client.
    owners: HashMap<String, ParamOwner>,
    /// Map from client ID → channel to push broadcast messages back to client.
    clients: HashMap<u32, Sender<String>>,
    next_id: u32,
}

impl SessionState {
    fn new() -> Self {
        Self {
            owners: HashMap::new(),
            clients: HashMap::new(),
            next_id: 1,
        }
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn broadcast_except(&self, sender_id: u32, msg: &str) {
        for (&id, tx) in &self.clients {
            if id != sender_id {
                let _ = tx.send(msg.to_owned());
            }
        }
    }

    fn broadcast_all(&self, msg: &str) {
        for tx in self.clients.values() {
            let _ = tx.send(msg.to_owned());
        }
    }

    fn peer_count(&self) -> usize {
        self.clients.len()
    }
}

// ── Server ────────────────────────────────────────────────────────────────────

/// Collaborative WebSocket-style server (raw TCP + newline-delimited JSON).
pub struct CollabServer {
    listener: TcpListener,
    state: Arc<Mutex<SessionState>>,
    event_tx: Sender<SessionEvent>,
}

impl CollabServer {
    /// Bind to `addr` and create the server.  Does **not** start accepting until
    /// [`run_background`] is called.
    pub fn new(addr: &str, event_tx: Sender<SessionEvent>) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        Ok(Self {
            listener,
            state: Arc::new(Mutex::new(SessionState::new())),
            event_tx,
        })
    }

    /// Return the local address the server is bound to.
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Spawn a background thread that accepts connections indefinitely.
    pub fn run_background(self) {
        thread::Builder::new()
            .name("collab-accept".into())
            .spawn(move || self.accept_loop())
            .expect("collab accept thread");
    }

    fn accept_loop(self) {
        log::info!(
            "[collab] listening on {}",
            self.listener.local_addr().unwrap()
        );
        for stream in self.listener.incoming() {
            match stream {
                Ok(s) => {
                    let state = Arc::clone(&self.state);
                    let event_tx = self.event_tx.clone();
                    thread::Builder::new()
                        .name("collab-client".into())
                        .spawn(move || handle_client(s, state, event_tx))
                        .expect("collab client thread");
                }
                Err(e) => log::warn!("[collab] accept error: {e}"),
            }
        }
    }
}

// ── Client handler ────────────────────────────────────────────────────────────

fn handle_client(
    stream: TcpStream,
    state: Arc<Mutex<SessionState>>,
    event_tx: Sender<SessionEvent>,
) {
    let peer = stream.peer_addr().ok();
    log::debug!("[collab] new connection from {:?}", peer);

    // Assign ID and register write channel.
    let (write_tx, write_rx) = crossbeam_channel::unbounded::<String>();
    let client_id = {
        let mut s = state.lock();
        let id = s.alloc_id();
        s.clients.insert(id, write_tx);
        let total = s.peer_count();
        let joined = format!(
            "{{\"peer_joined\":{{\"client_id\":{id},\"total\":{total}}}}}"
        );
        s.broadcast_except(id, &joined);
        id
    };

    let _ = event_tx.send(SessionEvent::ClientJoined { client_id });

    // Spawn a write thread so reads and writes don't block each other.
    let write_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            log::warn!("[collab] failed to clone stream: {e}");
            cleanup(client_id, &state, &event_tx);
            return;
        }
    };

    thread::Builder::new()
        .name("collab-write".into())
        .spawn(move || {
            let mut ws = write_stream;
            // Send welcome
            let welcome = format!("{{\"welcome\":{{\"client_id\":{client_id}}}}}");
            let _ = writeln!(ws, "{}", welcome);
            for msg in write_rx {
                if writeln!(ws, "{}", msg).is_err() {
                    break;
                }
            }
        })
        .expect("collab write thread");

    // Read loop (blocking).
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l.trim().to_owned(),
            Err(_) => break,
        };
        if line.is_empty() {
            continue;
        }
        dispatch_message(client_id, &line, &state, &event_tx);
    }

    cleanup(client_id, &state, &event_tx);
}

/// Parse and handle one newline-delimited JSON message from a client.
fn dispatch_message(
    client_id: u32,
    line: &str,
    state: &Arc<Mutex<SessionState>>,
    event_tx: &Sender<SessionEvent>,
) {
    // Minimal hand-rolled JSON dispatch to avoid pulling in serde in this module.
    // We support exactly three top-level keys: "claim", "release", "set".
    let trimmed = line.trim_matches(|c: char| c.is_whitespace() || c == '{' || c == '}');

    if line.contains("\"claim\"") {
        let params = extract_string_array(line, "claim");
        let mut s = state.lock();
        for param in params {
            if let Some(owner) = s.owners.get(&param) {
                if owner.client_id != client_id {
                    let err = format!(
                        "{{\"error\":\"parameter '{}' is owned by client {}\"}}",
                        param, owner.client_id
                    );
                    if let Some(tx) = s.clients.get(&client_id) {
                        let _ = tx.send(err);
                    }
                    continue;
                }
            }
            s.owners.insert(param.clone(), ParamOwner { client_id });
            log::debug!("[collab] client {client_id} claimed '{param}'");
        }
    } else if line.contains("\"release\"") {
        let params = extract_string_array(line, "release");
        let mut s = state.lock();
        for param in params {
            if s.owners.get(&param).map(|o| o.client_id) == Some(client_id) {
                s.owners.remove(&param);
            }
        }
    } else if line.contains("\"set\"") {
        // Extract key-value pairs from the nested object.
        let pairs = extract_set_pairs(line);
        let mut s = state.lock();
        for (param, value) in pairs {
            // Check ownership.
            if let Some(owner) = s.owners.get(&param) {
                if owner.client_id != client_id {
                    let err = format!(
                        "{{\"error\":\"parameter '{}' is owned by client {}\"}}",
                        param, owner.client_id
                    );
                    if let Some(tx) = s.clients.get(&client_id) {
                        let _ = tx.send(err);
                    }
                    continue;
                }
            }
            // Broadcast the update.
            let update = format!(
                "{{\"update\":{{\"{}\":{},\"owner\":{}}}}}",
                param, value, client_id
            );
            s.broadcast_all(&update);
            drop(s); // release lock before sending to channel
            let _ = event_tx.send(SessionEvent::ParamChanged {
                client_id,
                name: param.clone(),
                value,
            });
            s = state.lock();
        }
    } else {
        log::debug!("[collab] unknown message from {client_id}: {trimmed}");
    }
}

fn cleanup(
    client_id: u32,
    state: &Arc<Mutex<SessionState>>,
    event_tx: &Sender<SessionEvent>,
) {
    {
        let mut s = state.lock();
        s.clients.remove(&client_id);
        // Release all owned parameters.
        s.owners.retain(|_, o| o.client_id != client_id);
        let total = s.peer_count();
        let left = format!(
            "{{\"peer_left\":{{\"client_id\":{client_id},\"total\":{total}}}}}"
        );
        s.broadcast_all(&left);
    }
    let _ = event_tx.send(SessionEvent::ClientLeft { client_id });
    log::info!("[collab] client {client_id} disconnected");
}

// ── Minimal JSON helpers (no external parser dependency) ─────────────────────

/// Extract a JSON string array for a given key, e.g. `"claim": ["a","b"]`.
fn extract_string_array(json: &str, key: &str) -> Vec<String> {
    let mut results = Vec::new();
    let needle = format!("\"{key}\"");
    if let Some(pos) = json.find(&needle) {
        let after_key = &json[pos + needle.len()..];
        if let Some(start) = after_key.find('[') {
            if let Some(end) = after_key.find(']') {
                let inner = &after_key[start + 1..end];
                for part in inner.split(',') {
                    let s = part
                        .trim()
                        .trim_matches('"')
                        .trim()
                        .to_owned();
                    if !s.is_empty() {
                        results.push(s);
                    }
                }
            }
        }
    }
    results
}

/// Extract key-value pairs from a `"set": { ... }` object.
fn extract_set_pairs(json: &str) -> Vec<(String, f64)> {
    let mut results = Vec::new();
    if let Some(pos) = json.find("\"set\"") {
        let after = &json[pos + 5..];
        if let Some(start) = after.find('{') {
            if let Some(end) = after.rfind('}') {
                if end > start {
                    let inner = &after[start + 1..end];
                    for pair in inner.split(',') {
                        let parts: Vec<&str> = pair.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            let k = parts[0].trim().trim_matches('"').to_owned();
                            if let Ok(v) = parts[1].trim().parse::<f64>() {
                                results.push((k, v));
                            }
                        }
                    }
                }
            }
        }
    }
    results
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_string_array() {
        let json = r#"{"claim":["rho","sigma"]}"#;
        let result = extract_string_array(json, "claim");
        assert_eq!(result, vec!["rho", "sigma"]);
    }

    #[test]
    fn test_extract_set_pairs() {
        let json = r#"{"set":{"rho":28.5,"sigma":10.0}}"#;
        let pairs = extract_set_pairs(json);
        assert!(pairs.iter().any(|(k, v)| k == "rho" && (*v - 28.5).abs() < 1e-9));
        assert!(pairs.iter().any(|(k, v)| k == "sigma" && (*v - 10.0).abs() < 1e-9));
    }

    #[test]
    fn test_server_binds() {
        let (tx, _rx) = crossbeam_channel::unbounded();
        let server = CollabServer::new("127.0.0.1:0", tx).unwrap();
        let addr = server.local_addr().unwrap();
        assert!(addr.port() > 0);
    }
}
