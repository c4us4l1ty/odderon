//! Base64url without padding (RFC 4648 section 5), so a payload survives a
//! URL fragment and a QR byte segment without percent-escaping.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn encode_url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let word = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(word >> 18) as usize & 63] as char);
        out.push(ALPHABET[(word >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(word >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[word as usize & 63] as char);
        }
    }
    out
}

pub fn decode_url(text: &str) -> Option<Vec<u8>> {
    if text.len() % 4 == 1 {
        // One sextet cannot encode even one byte, even when all six bits
        // are zero. Accepting it creates a noncanonical duplicate encoding.
        return None;
    }
    if text.is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut accumulator = 0u32;
    let mut bits = 0u32;
    for character in text.bytes() {
        let value = ALPHABET.iter().position(|c| *c == character)? as u32;
        accumulator = (accumulator << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
        }
    }
    // Leftover bits must be zero padding, not hidden data.
    if bits > 0 && accumulator & ((1 << bits) - 1) != 0 {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{decode_url, encode_url};

    #[test]
    fn rejects_impossible_single_sextet_tails() {
        for n in 0..12 {
            let bytes: Vec<u8> = (0..n).collect();
            assert_eq!(decode_url(&encode_url(&bytes)), Some(bytes));
        }
        assert_eq!(decode_url("A"), None);
        assert_eq!(decode_url("AAAAA"), None);
        assert_eq!(decode_url("AB"), None); // nonzero padding bits
        assert_eq!(decode_url("AAB"), None);
    }
}
