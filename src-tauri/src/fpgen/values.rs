//! Value store for fpgen — maps base85-encoded indices to actual fingerprint values.
//!
//! Loads `values.json` (index of offsets/lengths) and `values.dat` (flat binary blob)
//! from zstd-compressed files embedded at compile time.

use super::base85;
use std::collections::HashMap;
use std::sync::OnceLock;

static VALUES: OnceLock<ValueStore> = OnceLock::new();

/// Stores the decompressed value blob and an offset index for O(1) lookups.
pub struct ValueStore {
  /// `entries[positional_index] = (byte_offset, byte_length)` into `data`.
  entries: Vec<(usize, usize)>,
  /// Decompressed contents of `values.dat`.
  data: Vec<u8>,
}

impl ValueStore {
  /// Get (or lazily initialise) the global singleton.
  pub fn global() -> &'static Self {
    VALUES.get_or_init(|| Self::load().expect("Failed to load fpgen value store"))
  }

  fn load() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
    let index_zst = include_bytes!("data/values.json.zst");
    let data_zst = include_bytes!("data/values.dat.zst");

    // Decompress both files.
    let index_bytes = zstd::bulk::decompress(index_zst, 1024 * 1024)?; // ~700 KB
    let data = zstd::bulk::decompress(data_zst, 256 * 1024 * 1024)?; // up to 256 MB

    // `values.json` is `{"hex_offset": length, ...}` in insertion order.
    // JSON object key order is not guaranteed, so we sort by offset to recover
    // the positional index (offsets are contiguous and monotonically increasing).
    let map: HashMap<String, usize> = serde_json::from_slice(&index_bytes)?;
    let mut entries: Vec<(usize, usize)> = map
      .into_iter()
      .map(|(hex, len)| {
        let offset = usize::from_str_radix(&hex, 16).unwrap_or(0);
        (offset, len)
      })
      .collect();
    entries.sort_unstable_by_key(|&(offset, _)| offset);

    log::info!(
      "fpgen: loaded {} value entries, data blob {} bytes",
      entries.len(),
      data.len()
    );

    Ok(Self { entries, data })
  }

  /// Look up the raw UTF-8 string at the given positional index.
  pub fn lookup_raw(&self, index: usize) -> Option<&str> {
    let &(offset, length) = self.entries.get(index)?;
    let end = offset + length;
    if end > self.data.len() {
      return None;
    }
    std::str::from_utf8(&self.data[offset..end]).ok()
  }

  /// Look up and parse the value at the given positional index as JSON.
  pub fn lookup_json(&self, index: usize) -> Option<serde_json::Value> {
    let raw = self.lookup_raw(index)?;
    serde_json::from_str(raw).ok()
  }

  /// Decode a base85-encoded key and return the parsed JSON value.
  pub fn decode_b85(&self, b85: &str) -> Option<serde_json::Value> {
    self.lookup_json(base85::to_index(b85))
  }

  /// Decode a base85-encoded key and return the raw string.
  pub fn decode_b85_raw(&self, b85: &str) -> Option<&str> {
    self.lookup_raw(base85::to_index(b85))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_load_value_store() {
    let store = ValueStore::global();
    assert!(
      store.entries.len() > 1000,
      "Expected many entries, got {}",
      store.entries.len()
    );
  }

  #[test]
  fn test_lookup_index_zero() {
    let store = ValueStore::global();
    let val = store.lookup_raw(0);
    assert!(val.is_some(), "Index 0 should exist");
  }

  #[test]
  fn test_decode_b85_bang() {
    let store = ValueStore::global();
    // "!" = index 0
    let val = store.decode_b85("!");
    assert!(val.is_some(), "b85 '!' should decode to index 0");
  }
}
