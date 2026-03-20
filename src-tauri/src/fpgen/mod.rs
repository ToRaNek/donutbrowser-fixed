//! fpgen — Realistic fingerprint generation based on 2.4M real browser sessions.
//!
//! Port of the Python `fpgen` library (scrapfly/fingerprint-generator) to Rust.
//! Uses a 110-node Bayesian network to produce fingerprints where every attribute
//! (UA, screen, GPU, fonts, audio, codecs, permissions, …) is statistically
//! consistent with real-world device data.
//!
//! # Usage
//!
//! ```ignore
//! let fp = fpgen::generate(Some(&FpgenOptions {
//!     browser: Some("Chrome".into()),
//!     platform: Some("windows".into()),
//!     ..Default::default()
//! }))?;
//! ```

mod base85;
pub mod convert;
mod network;
mod values;

use network::FpgenNetwork;
use std::collections::{HashMap, HashSet};
use values::ValueStore;

/// Options for constraining fingerprint generation.
#[derive(Debug, Clone, Default)]
pub struct FpgenOptions {
  /// Browser name filter (e.g. `"Chrome"`, `"Firefox"`, `"Edge"`).
  pub browser: Option<String>,
  /// Platform/OS filter (e.g. `"Windows"`, `"macOS"`, `"Linux"`).
  pub platform: Option<String>,
}

/// Generate a realistic fingerprint.
///
/// Returns a nested JSON object with paths like `navigator.userAgent`, `screen.width`,
/// `gpu.vendor`, `fonts`, `audio.*`, etc.
pub fn generate(options: Option<&FpgenOptions>) -> Result<serde_json::Value, FpgenError> {
  let network = FpgenNetwork::global();
  let store = ValueStore::global();

  let constraints = match options {
    Some(opts) => build_constraints(network, store, opts),
    None => HashMap::new(),
  };

  let sample = network.sample_with_constraints(&constraints);

  if sample.is_empty() {
    return Err(FpgenError::EmptySample);
  }

  Ok(FpgenNetwork::decode_sample(&sample))
}

/// Build constraint sets from user-friendly options.
///
/// Translates e.g. `browser: "Chrome"` into the set of base85 keys whose decoded
/// value contains "Chrome".
fn build_constraints(
  network: &FpgenNetwork,
  store: &ValueStore,
  opts: &FpgenOptions,
) -> HashMap<String, HashSet<String>> {
  let mut constraints = HashMap::new();

  if let Some(browser) = &opts.browser {
    if let Some(node) = network.get_node("browser") {
      let allowed = find_matching_keys(&node.possible_values, store, browser);
      if !allowed.is_empty() {
        constraints.insert("browser".to_string(), allowed);
      }
    }
  }

  if let Some(platform) = &opts.platform {
    // The platform info node name varies; try common names.
    for node_name in &["platformInfo", "platform", "navigator.platform"] {
      if let Some(node) = network.get_node(node_name) {
        let allowed = find_matching_keys(&node.possible_values, store, platform);
        if !allowed.is_empty() {
          constraints.insert(node_name.to_string(), allowed);
          break;
        }
      }
    }
  }

  constraints
}

/// Find all base85 keys whose decoded value contains `needle` (case-insensitive).
fn find_matching_keys(
  possible_values: &[String],
  store: &ValueStore,
  needle: &str,
) -> HashSet<String> {
  let needle_lower = needle.to_lowercase();
  possible_values
    .iter()
    .filter(|b85| {
      store
        .decode_b85_raw(b85)
        .map(|raw| raw.to_lowercase().contains(&needle_lower))
        .unwrap_or(false)
    })
    .cloned()
    .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum FpgenError {
  #[error("fpgen: generated sample was empty — constraints may be too restrictive")]
  EmptySample,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_generate_unconstrained() {
    let fp = generate(None).unwrap();
    assert!(fp.is_object());
    let obj = fp.as_object().unwrap();
    assert!(!obj.is_empty(), "Fingerprint should not be empty");
  }

  #[test]
  fn test_generate_macos() {
    let fp = generate(Some(&FpgenOptions {
      browser: Some("Chrome".into()),
      platform: Some("mac".into()),
    }))
    .unwrap();
    let obj = fp.as_object().unwrap();
    assert!(obj.get("navigator").is_some());
    assert!(obj.get("allFonts").is_some());
  }

  #[test]
  fn test_generate_chrome() {
    let fp = generate(Some(&FpgenOptions {
      browser: Some("Chrome".into()),
      ..Default::default()
    }))
    .unwrap();
    assert!(fp.is_object());
  }

  #[test]
  fn test_generate_firefox() {
    let fp = generate(Some(&FpgenOptions {
      browser: Some("Firefox".into()),
      ..Default::default()
    }))
    .unwrap();
    assert!(fp.is_object());
  }

  #[test]
  fn test_generate_windows() {
    let fp = generate(Some(&FpgenOptions {
      platform: Some("Windows".into()),
      ..Default::default()
    }))
    .unwrap();
    assert!(fp.is_object());
  }

  #[test]
  fn test_output_has_navigator() {
    let fp = generate(None).unwrap();
    let obj = fp.as_object().unwrap();
    assert!(
      obj.contains_key("navigator"),
      "Output should have navigator key. Keys: {:?}",
      obj.keys().collect::<Vec<_>>()
    );
  }
}
