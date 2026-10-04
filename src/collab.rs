//! Collaborative performance mode: a WebSocket server that lets several
//! people play one running instance together.
//!
//! Start the app with `math-sonify --collab 127.0.0.1:9001` (or
//! `0.0.0.0:9001` to accept other machines on your network), then connect
//! any WebSocket client: the bundled `collab.html` page, a browser console,
//! `websocat`, or your own code. Every value in `config.toml` is addressable
//! by its dotted path (`lorenz.rho`, `system.speed`, `audio.reverb_wet`,
//! `system.name`, ...), and changes are heard within one control tick.
//!
//! Each client may claim parameters; a claimed parameter can only be changed
//! by its owner until it is released or the owner disconnects.
//!
//! # Wire protocol
//!
//! One JSON object per WebSocket text message.
//!
//! ## Client to server
//! ```json
//! { "claim": ["lorenz.rho", "lorenz.sigma"] }
//! { "set": { "lorenz.rho": 28.5, "system.name": "rossler" } }
//! { "get": ["lorenz.rho"] }
//! { "release": ["lorenz.rho"] }
//! ```
//!
//! ## Server to client
//! ```json
//! { "welcome": { "client_id": 3, "peers": 2 } }
//! { "update": { "param": "lorenz.rho", "value": 28.5, "owner": 3 } }
//! { "values": { "lorenz.rho": 28.5 } }
//! { "claimed": ["lorenz.rho"] }
//! { "error": "parameter 'lorenz.rho' is owned by client 1" }
//! { "peer_joined": { "client_id": 4, "total": 2 } }
//! { "peer_left":   { "client_id": 4, "total": 1 } }
//! ```
//!
//! The server is plain threads over [`tungstenite`]: no async runtime.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use serde_json::{json, Map, Value};
use tungstenite::{Message, WebSocket};

/// Applies a change to the running app. Returns an error message for the
/// client when the path does not exist or the value has the wrong type.
pub type ApplyFn = dyn Fn(&str, &Value) -> Result<(), String> + Send + Sync;
/// Reads the current value at a dotted path.
pub type GetFn = dyn Fn(&str) -> Option<Value> + Send + Sync;

/// Events reported to the app as clients come, go and change things.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    /// A client changed a parameter (already applied).
    ParamChanged {
        client_id: u32,
        name: String,
        value: Value,
    },
    /// A client connected.
    ClientJoined { client_id: u32 },
    /// A client disconnected.
    ClientLeft { client_id: u32 },
}

struct SessionState {
    /// Parameter path to owning client.
    owners: HashMap<String, u32>,
    /// Client id to its outgoing message queue.
    clients: HashMap<u32, Sender<String>>,
    next_id: u32,
}

impl SessionState {
    fn send_to(&self, id: u32, msg: &Value) {
        if let Some(tx) = self.clients.get(&id) {
            let _ = tx.send(msg.to_string());
        }
    }

    fn broadcast(&self, msg: &Value, except: Option<u32>) {
        let text = msg.to_string();
        for (&id, tx) in &self.clients {
            if Some(id) != except {
                let _ = tx.send(text.clone());
            }
        }
    }
}

struct Shared {
    state: Mutex<SessionState>,
    apply: Box<ApplyFn>,
    get: Box<GetFn>,
    events: Sender<SessionEvent>,
}

/// The collaborative WebSocket server.
pub struct CollabServer {
    listener: TcpListener,
    shared: Arc<Shared>,
}

impl CollabServer {
    /// Bind to `addr`. Nothing is accepted until [`run_background`](Self::run_background).
    pub fn new(
        addr: &str,
        apply: Box<ApplyFn>,
        get: Box<GetFn>,
        events: Sender<SessionEvent>,
    ) -> std::io::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(addr)?,
            shared: Arc::new(Shared {
                state: Mutex::new(SessionState {
                    owners: HashMap::new(),
                    clients: HashMap::new(),
                    next_id: 1,
                }),
                apply,
                get,
                events,
            }),
        })
    }

    /// The address actually bound (useful with port 0).
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Accept connections on a background thread for the life of the process.
    pub fn run_background(self) -> std::io::Result<()> {
        thread::Builder::new()
            .name("collab-accept".into())
            .spawn(move || {
                for stream in self.listener.incoming() {
                    match stream {
                        Ok(s) => {
                            let shared = Arc::clone(&self.shared);
                            let spawned = thread::Builder::new()
                                .name("collab-client".into())
                                .spawn(move || handle_client(s, shared));
                            if let Err(e) = spawned {
                                log::warn!("[collab] could not start client thread: {e}");
                            }
                        }
                        Err(e) => log::warn!("[collab] accept error: {e}"),
                    }
                }
            })
            .map(|_| ())
    }
}

fn handle_client(stream: TcpStream, shared: Arc<Shared>) {
    let mut ws = match tungstenite::accept(stream) {
        Ok(ws) => ws,
        Err(e) => {
            log::debug!("[collab] handshake failed: {e}");
            return;
        }
    };
    // A short read timeout lets one thread both read and flush outgoing
    // messages without splitting the socket.
    let _ = ws.get_ref().set_read_timeout(Some(Duration::from_millis(20)));

    let (out_tx, out_rx) = crossbeam_channel::unbounded::<String>();
    let client_id = {
        let mut s = shared.state.lock();
        let id = s.next_id;
        s.next_id += 1;
        s.clients.insert(id, out_tx);
        let total = s.clients.len();
        s.send_to(id, &json!({ "welcome": { "client_id": id, "peers": total } }));
        s.broadcast(
            &json!({ "peer_joined": { "client_id": id, "total": total } }),
            Some(id),
        );
        id
    };
    let _ = shared.events.send(SessionEvent::ClientJoined { client_id });
    log::info!("[collab] client {client_id} connected");

    run_connection(&mut ws, &out_rx, client_id, &shared);

    {
        let mut s = shared.state.lock();
        s.clients.remove(&client_id);
        s.owners.retain(|_, owner| *owner != client_id);
        let total = s.clients.len();
        s.broadcast(
            &json!({ "peer_left": { "client_id": client_id, "total": total } }),
            None,
        );
    }
    let _ = shared.events.send(SessionEvent::ClientLeft { client_id });
    log::info!("[collab] client {client_id} disconnected");
}

fn run_connection(
    ws: &mut WebSocket<TcpStream>,
    out_rx: &Receiver<String>,
    client_id: u32,
    shared: &Shared,
) {
    loop {
        for text in out_rx.try_iter() {
            if ws.send(Message::text(text)).is_err() {
                return;
            }
        }
        match ws.read() {
            Ok(Message::Text(text)) => dispatch(client_id, text.as_str(), shared),
            Ok(Message::Close(_)) => {
                // Let tungstenite finish the closing handshake.
                let _ = ws.flush();
                return;
            }
            Ok(_) => {} // ping/pong handled by tungstenite; binary ignored
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                // Read timed out: flush pending pongs and loop.
                if ws.flush().is_err() {
                    return;
                }
            }
            Err(_) => return,
        }
    }
}

fn string_list(v: &Value) -> Vec<String> {
    match v {
        Value::Array(items) => items
            .iter()
            .filter_map(|i| i.as_str().map(str::to_owned))
            .collect(),
        Value::String(s) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// Handle one message from `client_id`.
fn dispatch(client_id: u32, text: &str, shared: &Shared) {
    let reply_err = |msg: String| {
        shared
            .state
            .lock()
            .send_to(client_id, &json!({ "error": msg }));
    };
    let msg: Map<String, Value> = match serde_json::from_str(text) {
        Ok(Value::Object(m)) => m,
        _ => return reply_err(format!("expected a JSON object, got {text:?}")),
    };

    if let Some(v) = msg.get("claim") {
        let mut s = shared.state.lock();
        let mut claimed = Vec::new();
        for param in string_list(v) {
            if (shared.get)(&param).is_none() {
                s.send_to(client_id, &json!({ "error": format!("unknown parameter '{param}'") }));
                continue;
            }
            match s.owners.get(&param) {
                Some(&owner) if owner != client_id => s.send_to(
                    client_id,
                    &json!({ "error": format!("parameter '{param}' is owned by client {owner}") }),
                ),
                _ => {
                    s.owners.insert(param.clone(), client_id);
                    claimed.push(param);
                }
            }
        }
        s.send_to(client_id, &json!({ "claimed": claimed }));
    }

    if let Some(v) = msg.get("release") {
        let mut s = shared.state.lock();
        for param in string_list(v) {
            if s.owners.get(&param) == Some(&client_id) {
                s.owners.remove(&param);
            }
        }
    }

    if let Some(v) = msg.get("set") {
        let Value::Object(pairs) = v else {
            return reply_err("\"set\" must be an object of path: value".into());
        };
        for (param, value) in pairs {
            let owner = shared.state.lock().owners.get(param).copied();
            if let Some(owner) = owner.filter(|&o| o != client_id) {
                reply_err(format!("parameter '{param}' is owned by client {owner}"));
                continue;
            }
            if let Err(e) = (shared.apply)(param, value) {
                reply_err(e);
                continue;
            }
            let now = (shared.get)(param).unwrap_or_else(|| value.clone());
            shared.state.lock().broadcast(
                &json!({ "update": { "param": param, "value": now, "owner": client_id } }),
                None,
            );
            let _ = shared.events.send(SessionEvent::ParamChanged {
                client_id,
                name: param.clone(),
                value: now,
            });
        }
    }

    if let Some(v) = msg.get("get") {
        let mut values = Map::new();
        for param in string_list(v) {
            match (shared.get)(&param) {
                Some(val) => {
                    values.insert(param, val);
                }
                None => reply_err(format!("unknown parameter '{param}'")),
            }
        }
        shared
            .state
            .lock()
            .send_to(client_id, &json!({ "values": values }));
    }

    if !["claim", "release", "set", "get"]
        .iter()
        .any(|k| msg.contains_key(*k))
    {
        reply_err("expected one of: claim, release, set, get".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    /// A server over a tiny in-memory parameter table.
    fn start() -> (SocketAddr, Arc<Mutex<Map<String, Value>>>, Receiver<SessionEvent>) {
        let table = Arc::new(Mutex::new(Map::new()));
        table.lock().insert("lorenz.rho".into(), json!(28.0));
        table.lock().insert("lorenz.sigma".into(), json!(10.0));
        let (t1, t2) = (Arc::clone(&table), Arc::clone(&table));
        let apply: Box<ApplyFn> = Box::new(move |p, v| {
            let mut t = t1.lock();
            match (t.get(p), v) {
                (Some(Value::Number(_)), Value::Number(_)) => {
                    t.insert(p.to_owned(), v.clone());
                    Ok(())
                }
                (Some(_), _) => Err(format!("{p}: wrong type")),
                (None, _) => Err(format!("unknown parameter '{p}'")),
            }
        });
        let get: Box<GetFn> = Box::new(move |p| t2.lock().get(p).cloned());
        let (tx, rx) = crossbeam_channel::unbounded();
        let server = CollabServer::new("127.0.0.1:0", apply, get, tx).unwrap();
        let addr = server.local_addr().unwrap();
        server.run_background().unwrap();
        (addr, table, rx)
    }

    fn connect(addr: SocketAddr) -> WebSocket<TcpStream> {
        let stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        tungstenite::client(format!("ws://{addr}"), stream).unwrap().0
    }

    /// Read messages until one has `key` at the top level.
    fn expect(
        ws: &mut WebSocket<TcpStream>,
        key: &str,
    ) -> Value {
        loop {
            let msg = ws.read().expect("message before timeout");
            if let Message::Text(t) = msg {
                let v: Value = serde_json::from_str(t.as_str()).unwrap();
                if v.get(key).is_some() {
                    return v[key].clone();
                }
            }
        }
    }

    fn send(ws: &mut WebSocket<TcpStream>, v: Value) {
        ws.send(Message::text(v.to_string())).unwrap();
    }

    #[test]
    fn set_applies_and_broadcasts_to_everyone() {
        let (addr, table, events) = start();
        let mut a = connect(addr);
        assert_eq!(expect(&mut a, "welcome")["client_id"], 1);
        let mut b = connect(addr);
        expect(&mut b, "welcome");
        expect(&mut a, "peer_joined");

        send(&mut a, json!({ "set": { "lorenz.rho": 99.5 } }));
        let ua = expect(&mut a, "update");
        let ub = expect(&mut b, "update");
        assert_eq!(ua, json!({ "param": "lorenz.rho", "value": 99.5, "owner": 1 }));
        assert_eq!(ua, ub);
        assert_eq!(table.lock()["lorenz.rho"], json!(99.5));
        assert!(events.iter().any(|e| matches!(
            e,
            SessionEvent::ParamChanged { client_id: 1, ref name, .. } if name == "lorenz.rho"
        )));
    }

    #[test]
    fn claimed_parameter_is_protected_until_owner_leaves() {
        let (addr, table, _events) = start();
        let mut a = connect(addr);
        expect(&mut a, "welcome");
        let mut b = connect(addr);
        expect(&mut b, "welcome");

        send(&mut a, json!({ "claim": ["lorenz.rho"] }));
        assert_eq!(expect(&mut a, "claimed"), json!(["lorenz.rho"]));
        send(&mut b, json!({ "set": { "lorenz.rho": 1.0 } }));
        let err = expect(&mut b, "error");
        assert!(err.as_str().unwrap().contains("owned by client 1"), "{err}");
        assert_eq!(table.lock()["lorenz.rho"], json!(28.0));

        drop(a);
        expect(&mut b, "peer_left");
        send(&mut b, json!({ "set": { "lorenz.rho": 1.0 } }));
        expect(&mut b, "update");
        assert_eq!(table.lock()["lorenz.rho"], json!(1.0));
    }

    #[test]
    fn bad_input_gets_an_error_not_a_crash() {
        let (addr, _table, _events) = start();
        let mut a = connect(addr);
        expect(&mut a, "welcome");
        send(&mut a, json!({ "set": { "lorenz.nope": 1.0 } }));
        assert!(expect(&mut a, "error").as_str().unwrap().contains("unknown"));
        a.send(Message::text("not json")).unwrap();
        expect(&mut a, "error");
        send(&mut a, json!({ "get": ["lorenz.sigma"] }));
        assert_eq!(expect(&mut a, "values"), json!({ "lorenz.sigma": 10.0 }));
    }
}
