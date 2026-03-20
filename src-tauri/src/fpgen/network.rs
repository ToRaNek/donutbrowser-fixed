//! Bayesian network for fpgen fingerprint generation.
//!
//! Loads the 110-node network trained on 2.4M real browser sessions and samples
//! consistent fingerprints via forward sampling with optional constraint filtering.

use super::values::ValueStore;
use rand::RngExt;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

static NETWORK: OnceLock<FpgenNetwork> = OnceLock::new();

// ---------------------------------------------------------------------------
// JSON deserialization types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct NetworkDef {
  nodes: Vec<NodeDef>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeDef {
  name: String,
  parent_names: Vec<String>,
  possible_values: Vec<String>,
  conditional_probabilities: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Runtime types
// ---------------------------------------------------------------------------

/// A single node in the Bayesian network.
pub struct FpgenNode {
  pub name: String,
  pub parent_names: Vec<String>,
  /// Possible values as base85-encoded index strings.
  pub possible_values: Vec<String>,
  /// Conditional probability table. Structure depends on the number of parents:
  ///   0 parents → `{ "val_b85": prob, … }`
  ///   N parents → nested N levels deep, keys = parent value b85, leaf = `{ "val_b85": prob }`
  cpt: serde_json::Value,
}

impl FpgenNode {
  /// Look up the probability distribution for this node given already-sampled parent values.
  ///
  /// Walks the CPT one parent at a time. Returns an empty map if any parent value is missing.
  pub fn get_distribution(&self, evidence: &HashMap<String, String>) -> HashMap<String, f64> {
    let mut current = &self.cpt;

    for parent_name in &self.parent_names {
      match evidence.get(parent_name) {
        Some(pv) => match current.get(pv.as_str()) {
          Some(next) => current = next,
          None => return HashMap::new(),
        },
        None => return HashMap::new(),
      }
    }

    // `current` should now be `{ "value_b85": probability, … }`.
    match current.as_object() {
      Some(obj) => obj
        .iter()
        .filter_map(|(k, v)| v.as_f64().map(|p| (k.clone(), p)))
        .collect(),
      None => HashMap::new(),
    }
  }
}

/// The full Bayesian network (110 nodes in topological order).
pub struct FpgenNetwork {
  nodes: Vec<FpgenNode>,
  node_indices: HashMap<String, usize>,
}

impl FpgenNetwork {
  /// Get (or lazily initialise) the global singleton.
  pub fn global() -> &'static Self {
    NETWORK.get_or_init(|| Self::load().expect("Failed to load fpgen network"))
  }

  fn load() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
    let zst = include_bytes!("data/fingerprint-network.json.zst");
    let json_bytes = zstd::bulk::decompress(zst, 10 * 1024 * 1024)?;
    let def: NetworkDef = serde_json::from_slice(&json_bytes)?;

    let mut nodes = Vec::with_capacity(def.nodes.len());
    let mut node_indices = HashMap::with_capacity(def.nodes.len());

    for (i, nd) in def.nodes.into_iter().enumerate() {
      node_indices.insert(nd.name.clone(), i);
      nodes.push(FpgenNode {
        name: nd.name,
        parent_names: nd.parent_names,
        possible_values: nd.possible_values,
        cpt: nd.conditional_probabilities,
      });
    }

    log::info!("fpgen: loaded network with {} nodes", nodes.len());
    Ok(Self {
      nodes,
      node_indices,
    })
  }

  /// Get a node by name.
  pub fn get_node(&self, name: &str) -> Option<&FpgenNode> {
    self.node_indices.get(name).map(|&i| &self.nodes[i])
  }

  /// Forward-sample with optional per-node constraints.
  ///
  /// `constraints` maps node names to the set of allowed base85-encoded values.
  /// For unconstrained nodes, all values in the CPT are eligible.
  pub fn sample_with_constraints(
    &self,
    constraints: &HashMap<String, HashSet<String>>,
  ) -> HashMap<String, String> {
    let mut evidence: HashMap<String, String> = HashMap::with_capacity(self.nodes.len());
    let mut rng = rand::rng();

    for node in &self.nodes {
      // If the user already fixed this node, honour it.
      if let Some(allowed) = constraints.get(&node.name) {
        if allowed.len() == 1 {
          let v = allowed.iter().next().unwrap().clone();
          evidence.insert(node.name.clone(), v);
          continue;
        }
      }

      let dist = node.get_distribution(&evidence);
      if dist.is_empty() {
        continue;
      }

      // Apply constraint filter.
      let filtered: HashMap<String, f64> = if let Some(allowed) = constraints.get(&node.name) {
        dist
          .into_iter()
          .filter(|(k, _)| allowed.contains(k))
          .collect()
      } else {
        dist
      };

      if filtered.is_empty() {
        continue;
      }

      let value = sample_from_distribution(&filtered, &mut rng);
      evidence.insert(node.name.clone(), value);
    }

    evidence
  }

  /// Decode a raw sample (base85 keys) into a nested JSON object using the value store.
  pub fn decode_sample(sample: &HashMap<String, String>) -> serde_json::Value {
    let store = ValueStore::global();
    let mut root = serde_json::Map::new();

    for (node_name, b85_value) in sample {
      let decoded = store
        .decode_b85(b85_value)
        .unwrap_or(serde_json::Value::Null);

      // Unflatten dot-separated paths: "navigator.userAgent" → {"navigator":{"userAgent": val}}
      let parts: Vec<&str> = node_name.split('.').collect();
      insert_nested(&mut root, &parts, decoded);
    }

    serde_json::Value::Object(root)
  }
}

/// Insert a value at a nested path in a JSON map.
fn insert_nested(
  map: &mut serde_json::Map<String, serde_json::Value>,
  path: &[&str],
  value: serde_json::Value,
) {
  match path.len() {
    0 => {}
    1 => {
      map.insert(path[0].to_string(), value);
    }
    _ => {
      let entry = map
        .entry(path[0].to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
      if let serde_json::Value::Object(child) = entry {
        insert_nested(child, &path[1..], value);
      }
    }
  }
}

/// Sample one value from a probability distribution using CDF inversion.
fn sample_from_distribution(dist: &HashMap<String, f64>, rng: &mut impl rand::Rng) -> String {
  let total: f64 = dist.values().sum();
  if total <= 0.0 {
    return dist.keys().next().cloned().unwrap_or_default();
  }

  let anchor: f64 = rng.random::<f64>() * total;
  let mut cumulative = 0.0;

  for (value, &prob) in dist {
    cumulative += prob;
    if cumulative > anchor {
      return value.clone();
    }
  }

  dist.keys().next().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_load_network() {
    let net = FpgenNetwork::global();
    assert!(
      net.nodes.len() > 50,
      "Expected many nodes, got {}",
      net.nodes.len()
    );
  }

  #[test]
  fn test_first_node_is_browser() {
    let net = FpgenNetwork::global();
    assert_eq!(
      net.nodes[0].name, "browser",
      "First node should be 'browser'"
    );
  }

  #[test]
  fn test_sample_produces_values() {
    let net = FpgenNetwork::global();
    let sample = net.sample_with_constraints(&HashMap::new());
    assert!(
      sample.len() > 50,
      "Sample should have many entries, got {}",
      sample.len()
    );
    assert!(sample.contains_key("browser"));
  }

  #[test]
  fn test_decode_sample() {
    let net = FpgenNetwork::global();
    let sample = net.sample_with_constraints(&HashMap::new());
    let decoded = FpgenNetwork::decode_sample(&sample);
    assert!(decoded.is_object());
    // Should have navigator, screen, etc.
    let obj = decoded.as_object().unwrap();
    assert!(
      obj.contains_key("navigator") || obj.contains_key("screen"),
      "Decoded sample should contain navigator or screen"
    );
  }

  #[test]
  fn test_sample_with_chrome_constraint() {
    let net = FpgenNetwork::global();
    let store = ValueStore::global();

    // Find the base85 key for "Chrome" in the browser node.
    let browser_node = net.get_node("browser").unwrap();
    let chrome_key = browser_node
      .possible_values
      .iter()
      .find(|v| {
        store
          .decode_b85_raw(v)
          .map(|s| s.contains("Chrome"))
          .unwrap_or(false)
      })
      .cloned();

    if let Some(key) = chrome_key {
      let mut constraints = HashMap::new();
      constraints.insert("browser".to_string(), HashSet::from([key]));
      let sample = net.sample_with_constraints(&constraints);
      let decoded = FpgenNetwork::decode_sample(&sample);
      let browser = decoded.get("browser");
      assert!(
        browser.is_some(),
        "Decoded sample should have browser field"
      );
    }
  }
}
