//! Specimen codes: a genome as a short base64 string, so a creature can be
//! copied out of one dish and dropped into another. Hand-rolled base64 to
//! keep the dependency list where it is.

use crate::genome::{Genome, N_W};

const MAGIC: &[u8; 4] = b"TDG1";
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

fn b64_decode(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for c in text.bytes() {
        if c.is_ascii_whitespace() || c == b'=' {
            continue;
        }
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return Err("not a genome code".into()),
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

pub fn encode_genome(g: &Genome) -> String {
    let mut bytes = Vec::with_capacity(4 + (Genome::N_TRAITS + N_W) * 4);
    bytes.extend_from_slice(MAGIC);
    for t in g.traits() {
        bytes.extend_from_slice(&t.to_le_bytes());
    }
    for w in g.weights {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    b64_encode(&bytes)
}

/// Decode and clamp. Any finite values are accepted and pulled into the
/// trait bounds, so a hand-edited code still yields a legal creature.
pub fn decode_genome(code: &str) -> Result<Genome, String> {
    let bytes = b64_decode(code.trim())?;
    let want = 4 + (Genome::N_TRAITS + N_W) * 4;
    if bytes.len() != want || &bytes[..4] != MAGIC {
        return Err("not a genome code".into());
    }
    let mut vals = Vec::with_capacity(Genome::N_TRAITS + N_W);
    for ch in bytes[4..].chunks_exact(4) {
        let v = f32::from_le_bytes([ch[0], ch[1], ch[2], ch[3]]);
        if !v.is_finite() {
            return Err("genome code holds a non-finite value".into());
        }
        vals.push(v);
    }
    let mut traits = [0.0f32; Genome::N_TRAITS];
    traits.copy_from_slice(&vals[..Genome::N_TRAITS]);
    let mut weights = [0.0f32; N_W];
    weights.copy_from_slice(&vals[Genome::N_TRAITS..]);
    Ok(Genome::from_parts(traits, weights).clamped())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn round_trip() {
        let mut rng = Rng::new(44);
        let g = Genome::random(&mut rng);
        let code = encode_genome(&g);
        assert!(code.len() > 600 && code.len() < 800, "{}", code.len());
        assert_eq!(decode_genome(&code).unwrap(), g);
        // Whitespace and line breaks are tolerated.
        let wrapped: String = code.chars().enumerate().map(|(i, c)| if i % 60 == 59 { format!("{c}\n") } else { c.to_string() }).collect();
        assert_eq!(decode_genome(&wrapped).unwrap(), g);
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode_genome("hello").is_err());
        assert!(decode_genome("").is_err());
        assert!(decode_genome(&b64_encode(b"TDG1short")).is_err());
        assert!(decode_genome("@@@@").is_err());
    }

    #[test]
    fn clamps_out_of_range() {
        let mut rng = Rng::new(45);
        let mut g = Genome::random(&mut rng);
        g.size = 99.0;
        g.diet = -3.0;
        g.weights[0] = 1e9;
        let d = decode_genome(&encode_genome(&g)).unwrap();
        assert_eq!(d.size, crate::genome::SIZE_MAX);
        assert_eq!(d.diet, 0.0);
        assert_eq!(d.weights[0], crate::genome::W_LIMIT);
    }
}
