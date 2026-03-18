/// OSC (Open Sound Control) packet encoding and UDP sender.
/// Implemented manually over std::net::UdpSocket — no external OSC crate required.

/// Pad a byte slice to the next multiple of 4 bytes.
fn pad4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

/// Encode a null-terminated, 4-byte-aligned OSC string.
fn encode_osc_string(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0); // null terminator
    pad4(&mut v);
    v
}

/// Encode an OSC packet with the given address and f32 arguments.
///
/// Format:
/// - Address string (null-terminated, padded to 4 bytes)
/// - Type tag string ",fff..." (one 'f' per arg, null-terminated, padded)
/// - Each float as 4 bytes big-endian
pub fn encode_osc(addr: &str, args: &[f32]) -> Vec<u8> {
    let mut packet = Vec::new();

    // Address string
    packet.extend(encode_osc_string(addr));

    // Type tag string: "," followed by one 'f' per argument
    let type_tag = format!(",{}", "f".repeat(args.len()));
    packet.extend(encode_osc_string(&type_tag));

    // Float arguments (big-endian)
    for &f in args {
        packet.extend_from_slice(&f.to_be_bytes());
    }

    packet
}

/// Sends OSC messages over UDP to a configurable target.
pub struct OscSender {
    socket: std::net::UdpSocket,
    target: String,
}

impl OscSender {
    /// Create a new OscSender bound to an ephemeral local port, targeting `host:port`.
    pub fn new(host: &str, port: u16) -> anyhow::Result<Self> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        let target = format!("{}:{}", host, port);
        Ok(Self { socket, target })
    }

    /// Send the current attractor state as an OSC message to `/sonify/state`.
    /// Arguments: x, y, z, speed, lyapunov (5 floats).
    pub fn send_state(&self, x: f32, y: f32, z: f32, speed: f32, lyapunov: f32) -> anyhow::Result<()> {
        let packet = encode_osc("/sonify/state", &[x, y, z, speed, lyapunov]);
        self.socket.send_to(&packet, &self.target)?;
        Ok(())
    }
}
