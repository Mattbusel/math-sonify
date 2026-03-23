// OSC collaborative synchronization module.
//
// Feature-flagged behind the `osc` Cargo feature.
// Provides:
//   - OscSyncServer  — listens on UDP 9001 for incoming OSC messages
//   - OscSyncClient  — broadcasts parameter / preset / beat messages
//   - CollaborativeSession — tracks peers, applies last-writer-wins updates
//
// OSC paths handled:
//   /mathsonify/param/{name}   f32  — set named parameter
//   /mathsonify/preset/{name}       — switch preset (no args)
//   /mathsonify/sync/beat      f32  — beat timestamp for tempo alignment
//
// Without the `osc` feature this module compiles to empty stubs so the rest
// of the codebase can still reference the types.

#![allow(dead_code)]

#[cfg(feature = "osc")]
pub mod inner {
    use std::collections::HashMap;
    use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use std::time::{Duration, Instant};

    use anyhow::Context as _;
    use parking_lot::Mutex;

    /// UDP port listened on by `OscSyncServer`.
    pub const OSC_LISTEN_PORT: u16 = 9001;
    /// Multicast group used by `OscSyncClient`.
    pub const OSC_MULTICAST_GROUP: &str = "239.0.0.1";
    /// Multicast port.
    pub const OSC_MULTICAST_PORT: u16 = 9001;

    // -----------------------------------------------------------------------
    // Decoded OSC message types
    // -----------------------------------------------------------------------

    /// An OSC message decoded from raw bytes.
    #[derive(Debug, Clone)]
    pub enum OscMessage {
        /// `/mathsonify/param/{name}` with value.
        Param { name: String, value: f32 },
        /// `/mathsonify/preset/{name}` — switch preset.
        Preset { name: String },
        /// `/mathsonify/sync/beat` with beat timestamp.
        Beat { timestamp: f32 },
        /// Unknown / unhandled address.
        Unknown { address: String },
    }

    // -----------------------------------------------------------------------
    // OscSyncServer
    // -----------------------------------------------------------------------

    /// Listens on `UDP 0.0.0.0:9001` for OSC messages from collaborating peers.
    pub struct OscSyncServer {
        socket: UdpSocket,
        running: Arc<AtomicBool>,
    }

    impl OscSyncServer {
        /// Bind to the OSC listen port (9001) and prepare for non-blocking receives.
        pub fn new() -> anyhow::Result<Self> {
            let socket = UdpSocket::bind(format!("0.0.0.0:{OSC_LISTEN_PORT}"))
                .with_context(|| "binding OSC listen socket on port {OSC_LISTEN_PORT}")?;
            socket.set_nonblocking(true)?;
            // Join multicast group so we receive multicast packets too.
            let multicast_addr: Ipv4Addr = OSC_MULTICAST_GROUP
                .parse()
                .with_context(|| "parsing OSC multicast group address")?;
            let _ = socket.join_multicast_v4(&multicast_addr, &Ipv4Addr::UNSPECIFIED);
            Ok(Self {
                socket,
                running: Arc::new(AtomicBool::new(true)),
            })
        }

        /// Poll for a waiting OSC message.  Returns `None` if no data is
        /// available yet (non-blocking).
        pub fn try_recv(&self) -> Option<(std::net::SocketAddr, OscMessage)> {
            let mut buf = [0u8; 1024];
            match self.socket.recv_from(&mut buf) {
                Ok((n, addr)) => {
                    let msg = decode_osc(&buf[..n]);
                    Some((addr, msg))
                }
                Err(_) => None,
            }
        }

        /// Stop the server.
        pub fn stop(&self) {
            self.running.store(false, Ordering::Relaxed);
        }
    }

    // -----------------------------------------------------------------------
    // OscSyncClient
    // -----------------------------------------------------------------------

    /// Broadcasts OSC messages to the multicast group.
    pub struct OscSyncClient {
        socket: UdpSocket,
        target: String,
    }

    impl OscSyncClient {
        /// Create a client that sends to the multicast group.
        pub fn new() -> anyhow::Result<Self> {
            let socket = UdpSocket::bind("0.0.0.0:0")?;
            socket.set_multicast_ttl_v4(1)?;
            socket.set_nonblocking(true)?;
            let target = format!("{OSC_MULTICAST_GROUP}:{OSC_MULTICAST_PORT}");
            Ok(Self { socket, target })
        }

        /// Broadcast a named parameter change to all peers.
        pub fn send_param(&self, name: &str, value: f32) -> anyhow::Result<()> {
            let addr = format!("/mathsonify/param/{name}");
            let pkt = encode_osc(&addr, &[value]);
            self.socket.send_to(&pkt, &self.target)?;
            Ok(())
        }

        /// Broadcast a preset switch to all peers.
        pub fn send_preset(&self, preset_name: &str) -> anyhow::Result<()> {
            let addr = format!("/mathsonify/preset/{preset_name}");
            let pkt = encode_osc(&addr, &[]);
            self.socket.send_to(&pkt, &self.target)?;
            Ok(())
        }

        /// Broadcast a beat sync signal.
        pub fn send_beat(&self, timestamp: f32) -> anyhow::Result<()> {
            let pkt = encode_osc("/mathsonify/sync/beat", &[timestamp]);
            self.socket.send_to(&pkt, &self.target)?;
            Ok(())
        }
    }

    // -----------------------------------------------------------------------
    // CollaborativeSession
    // -----------------------------------------------------------------------

    /// Information about a connected peer.
    #[derive(Debug, Clone)]
    pub struct PeerInfo {
        pub addr: std::net::SocketAddr,
        pub last_seen: Instant,
    }

    /// Conflict-resolution record for a parameter.
    #[derive(Debug, Clone)]
    struct ParamRecord {
        value: f32,
        /// Monotonic timestamp (from `Instant::now()`) of the last write.
        last_written: Instant,
    }

    /// Manages a collaborative session: tracks peers and applies
    /// last-writer-wins parameter updates.
    pub struct CollaborativeSession {
        peers: Mutex<HashMap<std::net::SocketAddr, PeerInfo>>,
        params: Mutex<HashMap<String, ParamRecord>>,
        /// Timeout after which a peer is considered disconnected.
        peer_timeout: Duration,
        /// Pending preset switch name (set by server, consumed by UI).
        pending_preset: Mutex<Option<String>>,
        /// Pending beat timestamp (for tempo alignment).
        pending_beat: Mutex<Option<f32>>,
    }

    impl CollaborativeSession {
        /// Create a new empty session.
        pub fn new() -> Self {
            Self {
                peers: Mutex::new(HashMap::new()),
                params: Mutex::new(HashMap::new()),
                peer_timeout: Duration::from_secs(10),
                pending_preset: Mutex::new(None),
                pending_beat: Mutex::new(None),
            }
        }

        /// Apply an incoming OSC message from `addr`.
        ///
        /// Uses last-writer-wins: if the incoming timestamp is newer than the
        /// recorded one, the value is accepted.
        pub fn apply(&self, addr: std::net::SocketAddr, msg: OscMessage) {
            // Update peer table.
            self.peers.lock().insert(addr, PeerInfo { addr, last_seen: Instant::now() });

            match msg {
                OscMessage::Param { name, value } => {
                    let mut params = self.params.lock();
                    let now = Instant::now();
                    let entry = params.entry(name).or_insert(ParamRecord {
                        value,
                        last_written: now,
                    });
                    // last-writer-wins: always accept since `now` is always "later"
                    // for a freshly received packet.
                    entry.value = value;
                    entry.last_written = now;
                }
                OscMessage::Preset { name } => {
                    *self.pending_preset.lock() = Some(name);
                }
                OscMessage::Beat { timestamp } => {
                    *self.pending_beat.lock() = Some(timestamp);
                }
                OscMessage::Unknown { .. } => {}
            }
        }

        /// Get the current value of a synced parameter, if any.
        pub fn get_param(&self, name: &str) -> Option<f32> {
            self.params.lock().get(name).map(|r| r.value)
        }

        /// Consume a pending preset switch (returns `Some` once per switch).
        pub fn take_pending_preset(&self) -> Option<String> {
            self.pending_preset.lock().take()
        }

        /// Consume a pending beat signal.
        pub fn take_pending_beat(&self) -> Option<f32> {
            self.pending_beat.lock().take()
        }

        /// Remove peers that haven't been heard from within the timeout.
        pub fn prune_stale_peers(&self) {
            let timeout = self.peer_timeout;
            self.peers.lock().retain(|_, p| p.last_seen.elapsed() < timeout);
        }

        /// Number of currently connected peers.
        pub fn peer_count(&self) -> usize {
            self.peers.lock().len()
        }
    }

    // -----------------------------------------------------------------------
    // OSC encode / decode helpers
    // -----------------------------------------------------------------------

    /// Pad a buffer to the next 4-byte boundary.
    fn pad4(v: &mut Vec<u8>) {
        while v.len() % 4 != 0 {
            v.push(0);
        }
    }

    fn encode_osc_string(s: &str) -> Vec<u8> {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        pad4(&mut v);
        v
    }

    /// Encode an OSC message with the given address and f32 arguments.
    pub fn encode_osc(addr: &str, args: &[f32]) -> Vec<u8> {
        let mut pkt = Vec::new();
        pkt.extend(encode_osc_string(addr));
        let tag = format!(",{}", "f".repeat(args.len()));
        pkt.extend(encode_osc_string(&tag));
        for &f in args {
            pkt.extend_from_slice(&f.to_be_bytes());
        }
        pkt
    }

    /// Decode an incoming OSC packet to an [`OscMessage`].
    pub fn decode_osc(data: &[u8]) -> OscMessage {
        // Minimal OSC decoder: parse address string, then type tag, then floats.
        fn read_osc_string(data: &[u8], pos: usize) -> Option<(String, usize)> {
            let end = data[pos..].iter().position(|&b| b == 0)?;
            let s = std::str::from_utf8(&data[pos..pos + end]).ok()?.to_owned();
            let raw_len = end + 1;
            let padded = (raw_len + 3) & !3;
            Some((s, pos + padded))
        }

        let (addr, mut pos) = match read_osc_string(data, 0) {
            Some(v) => v,
            None => return OscMessage::Unknown { address: String::new() },
        };

        // Parse type tag string.
        let (tags, next_pos) = match read_osc_string(data, pos) {
            Some(v) => v,
            None => return OscMessage::Unknown { address: addr },
        };
        pos = next_pos;

        // Read f32 arguments (tag chars 'f').
        let mut floats: Vec<f32> = Vec::new();
        for c in tags.chars().skip(1) {
            // skip leading ','
            if c == 'f' && pos + 4 <= data.len() {
                let bytes = [data[pos], data[pos + 1], data[pos + 2], data[pos + 3]];
                floats.push(f32::from_be_bytes(bytes));
                pos += 4;
            }
        }

        // Match address pattern.
        if let Some(name) = addr.strip_prefix("/mathsonify/param/") {
            let value = floats.first().copied().unwrap_or(0.0);
            return OscMessage::Param { name: name.to_owned(), value };
        }
        if let Some(name) = addr.strip_prefix("/mathsonify/preset/") {
            return OscMessage::Preset { name: name.to_owned() };
        }
        if addr == "/mathsonify/sync/beat" {
            let timestamp = floats.first().copied().unwrap_or(0.0);
            return OscMessage::Beat { timestamp };
        }

        OscMessage::Unknown { address: addr }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_encode_decode_param() {
            let pkt = encode_osc("/mathsonify/param/reverb_wet", &[0.42]);
            let msg = decode_osc(&pkt);
            match msg {
                OscMessage::Param { name, value } => {
                    assert_eq!(name, "reverb_wet");
                    assert!((value - 0.42).abs() < 1e-5, "value mismatch: {value}");
                }
                other => panic!("unexpected: {other:?}"),
            }
        }

        #[test]
        fn test_encode_decode_preset() {
            let pkt = encode_osc("/mathsonify/preset/Lorenz Ambience", &[]);
            let msg = decode_osc(&pkt);
            match msg {
                OscMessage::Preset { name } => assert_eq!(name, "Lorenz Ambience"),
                other => panic!("unexpected: {other:?}"),
            }
        }

        #[test]
        fn test_encode_decode_beat() {
            let pkt = encode_osc("/mathsonify/sync/beat", &[128.0]);
            let msg = decode_osc(&pkt);
            match msg {
                OscMessage::Beat { timestamp } => {
                    assert!((timestamp - 128.0).abs() < 1e-5);
                }
                other => panic!("unexpected: {other:?}"),
            }
        }

        #[test]
        fn test_collaborative_session_param_lww() {
            let session = CollaborativeSession::new();
            let addr: std::net::SocketAddr = "127.0.0.1:12345".parse().unwrap();
            session.apply(addr, OscMessage::Param { name: "sigma".into(), value: 12.0 });
            assert_eq!(session.get_param("sigma"), Some(12.0));
            session.apply(addr, OscMessage::Param { name: "sigma".into(), value: 20.0 });
            assert_eq!(session.get_param("sigma"), Some(20.0));
        }

        #[test]
        fn test_collaborative_session_peer_count() {
            let session = CollaborativeSession::new();
            let a: std::net::SocketAddr = "127.0.0.1:1111".parse().unwrap();
            let b: std::net::SocketAddr = "127.0.0.1:2222".parse().unwrap();
            session.apply(a, OscMessage::Beat { timestamp: 0.0 });
            session.apply(b, OscMessage::Beat { timestamp: 0.0 });
            assert_eq!(session.peer_count(), 2);
        }

        #[test]
        fn test_collaborative_session_pending_preset() {
            let session = CollaborativeSession::new();
            let addr: std::net::SocketAddr = "127.0.0.1:9999".parse().unwrap();
            session.apply(addr, OscMessage::Preset { name: "FM Chaos".into() });
            assert_eq!(session.take_pending_preset(), Some("FM Chaos".into()));
            assert_eq!(session.take_pending_preset(), None);
        }
    }
}

// ---------------------------------------------------------------------------
// Stub re-exports when the feature is not enabled.
// ---------------------------------------------------------------------------

#[cfg(not(feature = "osc"))]
pub mod inner {
    /// Stub server — no-ops when the `osc` feature is disabled.
    pub struct OscSyncServer;
    impl OscSyncServer {
        pub fn new() -> anyhow::Result<Self> {
            anyhow::bail!("OSC sync requires the `osc` Cargo feature")
        }
    }

    /// Stub client — no-ops when the `osc` feature is disabled.
    pub struct OscSyncClient;
    impl OscSyncClient {
        pub fn new() -> anyhow::Result<Self> {
            anyhow::bail!("OSC sync requires the `osc` Cargo feature")
        }
    }

    /// Stub session — always returns zero peers.
    pub struct CollaborativeSession;
    impl CollaborativeSession {
        pub fn new() -> Self { Self }
        pub fn peer_count(&self) -> usize { 0 }
    }
}

#[allow(unused_imports)]
pub use inner::{CollaborativeSession, OscSyncClient, OscSyncServer};
