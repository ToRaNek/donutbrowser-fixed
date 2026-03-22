//! donut-cli: CLI tool to generate Camoufox fingerprint configurations.
//!
//! Uses the same fpgen + CamoufoxConfigBuilder pipeline as the main Donut Browser
//! to produce CAMOU_CONFIG environment variables that can be passed to Camoufox.
//!
//! # Usage
//!
//! ```bash
//! donut-cli generate-fingerprint --os macos
//! donut-cli generate-fingerprint --os macos --count 2
//! ```

use clap::{Arg, Command};
use std::collections::HashMap;

fn main() {
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
    .format_timestamp(None)
    .init();

  let matches = Command::new("donut-cli")
    .version("0.1.0")
    .about("Donut Browser CLI - fingerprint config generator")
    .subcommand(
      Command::new("generate-fingerprint")
        .about("Generate Camoufox fingerprint configuration")
        .arg(
          Arg::new("os")
            .long("os")
            .help("Target operating system: macos, windows, linux")
            .default_value("macos"),
        )
        .arg(
          Arg::new("count")
            .long("count")
            .short('n')
            .help("Number of fingerprint configs to generate")
            .default_value("1"),
        )
        .arg(
          Arg::new("export")
            .long("export")
            .help("Output as shell export commands instead of JSON")
            .action(clap::ArgAction::SetTrue),
        )
        .arg(
          Arg::new("diff")
            .long("diff")
            .help("When count > 1, show which keys differ between profiles")
            .action(clap::ArgAction::SetTrue),
        )
        .arg(
          Arg::new("ff-version")
            .long("ff-version")
            .help("Pin Firefox version number (e.g. 135)")
            .value_parser(clap::value_parser!(u32)),
        ),
    )
    .get_matches();

  match matches.subcommand() {
    Some(("generate-fingerprint", sub_matches)) => {
      let os = sub_matches.get_one::<String>("os").unwrap();
      let count: usize = sub_matches
        .get_one::<String>("count")
        .unwrap()
        .parse()
        .expect("count must be a number");
      let export_mode = sub_matches.get_flag("export");
      let diff_mode = sub_matches.get_flag("diff");
      let ff_version = sub_matches.get_one::<u32>("ff-version").copied();

      generate_fingerprints(os, count, export_mode, diff_mode, ff_version);
    }
    _ => {
      eprintln!("No subcommand provided. Use --help for usage.");
      std::process::exit(1);
    }
  }
}

fn generate_fingerprints(
  os: &str,
  count: usize,
  export_mode: bool,
  diff_mode: bool,
  ff_version: Option<u32>,
) {
  let mut all_configs: Vec<HashMap<String, serde_json::Value>> = Vec::new();
  let mut all_env_vars: Vec<HashMap<String, String>> = Vec::new();
  let mut results: Vec<serde_json::Value> = Vec::new();

  for i in 0..count {
    // Build config using CamoufoxConfigBuilder
    let mut builder = donutbrowser_lib::camoufox::config::CamoufoxConfigBuilder::new()
      .operating_system(os)
      .block_images(false)
      .block_webrtc(false)
      .block_webgl(false);

    if let Some(ver) = ff_version {
      builder = builder.ff_version(ver);
    }

    let config = builder.build().unwrap_or_else(|e| {
      eprintln!("Failed to build config for profile {}: {}", i, e);
      std::process::exit(1);
    });

    // Convert to env vars
    let env_vars =
      donutbrowser_lib::camoufox::env_vars::config_to_env_vars(&config.fingerprint_config)
        .unwrap_or_else(|e| {
          eprintln!("Failed to convert to env vars: {}", e);
          std::process::exit(1);
        });

    if export_mode {
      // Print shell export commands
      if count > 1 {
        println!("# --- Profile {} ---", i);
      }
      let mut sorted_env: Vec<_> = env_vars.iter().collect();
      sorted_env.sort_by_key(|(k, _)| (*k).clone());
      for (key, value) in &sorted_env {
        // Escape single quotes in the value for shell safety
        let escaped = value.replace('\'', "'\\''");
        println!("export {}='{}'", key, escaped);
      }
      if count > 1 {
        println!();
      }
    } else {
      // Build JSON output
      let json = serde_json::json!({
          "profile": i,
          "env_vars": env_vars,
          "config": config.fingerprint_config,
          "target_os": config.target_os,
      });
      results.push(json);
    }

    all_configs.push(config.fingerprint_config);
    all_env_vars.push(env_vars);
  }

  if !export_mode {
    if count == 1 {
      println!("{}", serde_json::to_string_pretty(&results[0]).unwrap());
    } else {
      println!("{}", serde_json::to_string_pretty(&results).unwrap());
    }
  }

  // Show diff when count > 1 and diff mode is enabled
  if diff_mode && count > 1 {
    show_diff(&all_configs);
  }
}

fn show_diff(configs: &[HashMap<String, serde_json::Value>]) {
  use std::collections::HashSet;

  let all_keys: HashSet<String> = configs.iter().flat_map(|c| c.keys().cloned()).collect();

  let mut identical_keys = Vec::new();
  let mut different_keys = Vec::new();

  for key in &all_keys {
    let values: Vec<Option<&serde_json::Value>> = configs.iter().map(|c| c.get(key)).collect();
    let first = values[0];
    let all_same = values.iter().all(|v| v == &first);

    if all_same {
      identical_keys.push(key.clone());
    } else {
      different_keys.push(key.clone());
    }
  }

  identical_keys.sort();
  different_keys.sort();

  eprintln!();
  eprintln!(
    "=== DIFF: {} profiles, {} total keys ===",
    configs.len(),
    all_keys.len()
  );
  eprintln!(
    "Identical: {} ({:.0}%)  |  Different: {} ({:.0}%)",
    identical_keys.len(),
    100.0 * identical_keys.len() as f64 / all_keys.len() as f64,
    different_keys.len(),
    100.0 * different_keys.len() as f64 / all_keys.len() as f64,
  );

  eprintln!();
  eprintln!("--- DIFFERENT KEYS ---");
  for key in &different_keys {
    eprintln!("  {}:", key);
    for (i, config) in configs.iter().enumerate() {
      let val = config
        .get(key)
        .map(|v| {
          let s = v.to_string();
          if s.len() > 80 {
            format!("{}...", &s[..80])
          } else {
            s
          }
        })
        .unwrap_or_else(|| "MISSING".to_string());
      eprintln!("    profile {}: {}", i, val);
    }
  }

  eprintln!();
  eprintln!("--- IDENTICAL KEYS ({}) ---", identical_keys.len());
  for key in &identical_keys {
    let val = configs[0]
      .get(key)
      .map(|v| {
        let s = v.to_string();
        if s.len() > 80 {
          format!("{}...", &s[..80])
        } else {
          s
        }
      })
      .unwrap_or_else(|| "MISSING".to_string());
    eprintln!("  {} = {}", key, val);
  }

  // Critical signals check
  let critical_keys = [
    "navigator.userAgent",
    "navigator.hardwareConcurrency",
    "screen.width",
    "screen.height",
    "window.innerWidth",
    "window.innerHeight",
    "window.outerWidth",
    "window.outerHeight",
    "canvas:aaOffset",
    "fonts:spacing_seed",
  ];

  eprintln!();
  eprintln!("--- CRITICAL SIGNAL CHECK ---");
  let mut critical_different = 0;
  for key in &critical_keys {
    let values: Vec<Option<&serde_json::Value>> = configs.iter().map(|c| c.get(*key)).collect();
    let first = values[0];
    let all_same = values.iter().all(|v| v == &first);
    let status = if all_same {
      "IDENTICAL"
    } else {
      critical_different += 1;
      "DIFFERENT"
    };
    eprintln!("  {}: {}", key, status);
  }
  eprintln!(
    "  => {}/{} critical signals differ",
    critical_different,
    critical_keys.len()
  );
}
