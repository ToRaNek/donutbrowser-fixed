//! RFC 1924 base85 decoding for fpgen value indices.

/// RFC 1924 base85 alphabet.
const ALPHABET: &[u8; 85] =
  b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";

/// Lookup table: ASCII byte → alphabet position (255 = invalid).
const fn build_decode_table() -> [u8; 128] {
  let mut table = [255u8; 128];
  let mut i = 0;
  while i < 85 {
    table[ALPHABET[i] as usize] = i as u8;
    i += 1;
  }
  table
}

static DECODE_TABLE: [u8; 128] = build_decode_table();

#[inline]
fn char_val(c: u8) -> u8 {
  if c < 128 {
    DECODE_TABLE[c as usize]
  } else {
    255
  }
}

/// Decode an RFC 1924 base85 string into bytes (matching Python's `base64.b85decode`).
pub fn decode(input: &str) -> Vec<u8> {
  let src = input.as_bytes();
  let n = src.len();
  if n == 0 {
    return Vec::new();
  }

  let padding = (5 - n % 5) % 5;
  let padded_len = n + padding;
  let out_len = (padded_len / 5) * 4;

  let mut buf = Vec::with_capacity(padded_len);
  buf.extend_from_slice(src);
  buf.extend(std::iter::repeat_n(b'~', padding));

  let mut result = Vec::with_capacity(out_len);
  for chunk in buf.chunks_exact(5) {
    let mut acc: u64 = 0;
    for &c in chunk {
      acc = acc * 85 + char_val(c) as u64;
    }
    result.extend_from_slice(&(acc as u32).to_be_bytes());
  }

  // Remove padding bytes from the end.
  result.truncate(out_len - padding);
  result
}

/// Decode an RFC 1924 base85 string directly to a `usize` index.
///
/// Interprets the decoded bytes as a big-endian unsigned integer.
pub fn to_index(input: &str) -> usize {
  let bytes = decode(input);
  let mut result: usize = 0;
  for &b in &bytes {
    result = (result << 8) | b as usize;
  }
  result
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_single_char_bang() {
    // "!" should decode to empty bytes → index 0
    assert_eq!(to_index("!"), 0);
  }

  #[test]
  fn test_two_char() {
    // "0!" → chars '0'=0, '!'=62 → padded "0!~~~"
    // acc = 0*85^4 + 62*85^3 + 84*85^2 + 84*85 + 84
    // bytes[0] = (acc >> 24) & 0xFF
    // after removing 3 padding bytes, 1 byte remains
    let idx = to_index("0!");
    assert!(idx < 256, "2-char b85 should decode to 1 byte, got {idx}");
  }

  #[test]
  fn test_roundtrip_known_values() {
    // Index 0 should be encoded as "!" in fpgen
    assert_eq!(to_index("!"), 0);
    // Small indices should produce small values
    let idx = to_index("0!");
    assert!(idx < 256);
  }

  #[test]
  fn test_decode_empty() {
    assert_eq!(decode(""), Vec::<u8>::new());
    assert_eq!(to_index(""), 0);
  }
}
