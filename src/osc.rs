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
    pub fn send_state(
        &self,
        x: f32,
        y: f32,
        z: f32,
        speed: f32,
        lyapunov: f32,
    ) -> anyhow::Result<()> {
        let packet = encode_osc("/sonify/state", &[x, y, z, speed, lyapunov]);
        self.socket.send_to(&packet, &self.target)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pad4_already_aligned() {
        let mut v = vec![1u8, 2, 3, 4];
        pad4(&mut v);
        assert_eq!(v.len(), 4, "already-aligned vec should not be padded");
    }

    #[test]
    fn test_pad4_unaligned() {
        let mut v = vec![1u8, 2, 3];
        pad4(&mut v);
        assert_eq!(v.len(), 4, "3-byte vec should be padded to 4");
        assert_eq!(v[3], 0, "padding byte should be zero");
    }

    #[test]
    fn test_encode_osc_string_null_terminated_and_padded() {
        // "/ab" + null = 4 bytes → no extra padding needed
        let s = encode_osc_string("/ab");
        assert_eq!(s.len() % 4, 0, "encoded string must be a multiple of 4 bytes");
        assert_eq!(s[3], 0, "byte after string should be null");
    }

    #[test]
    fn test_encode_osc_string_empty() {
        let s = encode_osc_string("");
        assert_eq!(s.len() % 4, 0, "empty string encoding must be 4-byte aligned");
        assert_eq!(s[0], 0, "first byte of empty string should be null terminator");
    }

    #[test]
    fn test_encode_osc_no_args_packet_length() {
        // Address "/x" = 4 bytes (2 chars + null + 1 pad), type tag "," = 4 bytes
        let packet = encode_osc("/x", &[]);
        assert_eq!(packet.len() % 4, 0, "packet must be 4-byte aligned");
        // Should contain address and type tag only
        assert!(packet.len() >= 8, "packet must include address and type tag");
    }

    #[test]
    fn test_encode_osc_single_float_correct_bytes() {
        // Encode 1.0f32 as big-endian: 0x3F800000
        let packet = encode_osc("/f", &[1.0_f32]);
        let expected = 1.0f32.to_be_bytes();
        // Last 4 bytes of the packet should be the float
        let float_bytes = &packet[packet.len() - 4..];
        assert_eq!(float_bytes, &expected, "float should be encoded big-endian");
    }

    #[test]
    fn test_encode_osc_packet_length_grows_with_args() {
        let p0 = encode_osc("/s", &[]);
        let p3 = encode_osc("/s", &[1.0, 2.0, 3.0]);
        // Each extra float adds 4 bytes; type tag also grows but stays 4-byte aligned
        assert!(p3.len() > p0.len(), "packet with 3 floats should be longer than packet with 0");
        assert_eq!(p3.len() % 4, 0, "packet must remain 4-byte aligned");
    }
}
