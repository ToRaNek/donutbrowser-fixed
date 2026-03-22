//! Camoufox configuration builder.
//!
//! Converts fingerprints to Camoufox configuration format and builds launch options.

use rand::RngExt;
use serde_yaml;
use std::collections::HashMap;
use std::path::Path;

use crate::camoufox::data;
use crate::camoufox::env_vars;
use crate::camoufox::fingerprint::types::*;
use crate::camoufox::fonts;
use crate::camoufox::geolocation;
use crate::camoufox::webgl;

/// Browserforge mapping from YAML.
type BrowserforgeMapping = HashMap<String, serde_yaml::Value>;

/// Load the browserforge mapping from embedded YAML.
fn load_browserforge_mapping() -> BrowserforgeMapping {
  serde_yaml::from_str(data::BROWSERFORGE_YML).unwrap_or_default()
}

/// Convert a fingerprint to Camoufox configuration.
pub fn from_browserforge(
  fingerprint: &Fingerprint,
  ff_version: Option<u32>,
) -> HashMap<String, serde_json::Value> {
  let mapping = load_browserforge_mapping();
  let mut config = HashMap::new();

  // Convert fingerprint to a JSON value for easier traversal
  let fp_json = serde_json::to_value(fingerprint).unwrap_or_default();

  // Apply mappings recursively
  cast_to_properties(&mut config, &mapping, &fp_json, ff_version);

  // Handle window.screenX and window.screenY
  handle_screen_xy(&mut config, &fingerprint.screen);

  config
}

/// Recursively cast fingerprint properties to Camoufox config format.
fn cast_to_properties(
  config: &mut HashMap<String, serde_json::Value>,
  mapping: &BrowserforgeMapping,
  fingerprint: &serde_json::Value,
  ff_version: Option<u32>,
) {
  if let serde_json::Value::Object(fp_obj) = fingerprint {
    for (key, mapping_value) in mapping {
      let fp_value = fp_obj.get(key);

      match mapping_value {
        serde_yaml::Value::String(target_key) => {
          if let Some(value) = fp_value {
            let mut final_value = value.clone();

            // Handle negative screen values
            if target_key.starts_with("screen.") {
              if let Some(num) = final_value.as_i64() {
                if num < 0 {
                  final_value = serde_json::json!(0);
                }
              }
            }

            // Replace Firefox version in user agent strings
            if let (Some(version), Some(s)) = (ff_version, final_value.as_str()) {
              let replaced = replace_ff_version(s, version);
              final_value = serde_json::json!(replaced);
            }

            config.insert(target_key.clone(), final_value);
          }
        }
        serde_yaml::Value::Mapping(nested_mapping) => {
          if let Some(nested_fp) = fp_value {
            let nested: BrowserforgeMapping = nested_mapping
              .iter()
              .filter_map(|(k, v)| k.as_str().map(|ks| (ks.to_string(), v.clone())))
              .collect();
            cast_to_properties(config, &nested, nested_fp, ff_version);
          }
        }
        _ => {}
      }
    }
  }
}

/// Replace Firefox version in user agent and related strings.
fn replace_ff_version(s: &str, version: u32) -> String {
  // Match patterns like "135.0" (Firefox version) and replace with new version
  let re = regex_lite::Regex::new(r"(?<!\d)(1[0-9]{2})(\.0)(?!\d)").unwrap_or_else(|_| {
    // Fallback - just do simple replacement
    regex_lite::Regex::new(r"Firefox/\d+").unwrap()
  });

  re.replace_all(s, format!("{}.0", version).as_str())
    .to_string()
}

/// Diversify a Firefox user-agent string to ensure per-profile uniqueness.
///
/// fpgen's Bayesian network tends to produce the same high-probability UA for a
/// given OS/browser combination. This function adds realistic variation by:
/// 1. Randomizing the Firefox `rv:XX.0` / `Firefox/XX.0` version within a
///    recent release range (130-146 unless `ff_version` is pinned).
/// 2. For macOS UAs, varying the `Intel Mac OS X 10.XX` version string to
///    use different plausible macOS versions.
fn diversify_user_agent(ua: &str, pinned_version: Option<u32>, rng: &mut impl rand::Rng) -> String {
  use rand::RngExt;
  let mut result = ua.to_string();

  // 1. Randomize Firefox version unless a specific version was pinned
  if pinned_version.is_none() {
    // Pick a plausible recent Firefox version (130-146)
    let random_ver: u32 = rng.random_range(130..=146);

    // Replace rv:XXX.0 and Firefox/XXX.0 patterns
    let re = regex_lite::Regex::new(r"rv:(\d{3})\.0").unwrap();
    result = re
      .replace_all(&result, format!("rv:{random_ver}.0").as_str())
      .to_string();

    let re2 = regex_lite::Regex::new(r"Firefox/(\d{3})\.0").unwrap();
    result = re2
      .replace_all(&result, format!("Firefox/{random_ver}.0").as_str())
      .to_string();
  }

  // 2. For macOS UAs, vary the OS version string.
  //    Real macOS versions in the wild: 10.13 (High Sierra) through 10.15 (Catalina),
  //    plus 11.x-14.x for Big Sur through Sonoma.  Firefox UA always uses
  //    "Intel Mac OS X 10.XX" format even on Apple Silicon for compatibility.
  if result.contains("Mac OS X") {
    let macos_versions = [
      "10.13", "10.14", "10.15", // High Sierra, Mojave, Catalina
      "10.15", "10.15", // Catalina is most common (weighted)
    ];
    let chosen = macos_versions[rng.random_range(0..macos_versions.len())];

    let re_mac = regex_lite::Regex::new(r"Mac OS X \d+\.\d+").unwrap();
    result = re_mac
      .replace_all(&result, format!("Mac OS X {chosen}").as_str())
      .to_string();
  }

  // 3. For Windows UAs, vary the Windows NT version.
  //    Windows 10 (NT 10.0) and Windows 11 (NT 10.0) use the same NT string,
  //    but we can also include Windows 8.1 (NT 6.3) at low frequency.
  if result.contains("Windows NT") {
    let nt_versions = [
      "10.0", "10.0", "10.0", "10.0", // Windows 10/11 dominant
      "6.3",  // Windows 8.1 at low frequency
    ];
    let chosen = nt_versions[rng.random_range(0..nt_versions.len())];

    let re_nt = regex_lite::Regex::new(r"Windows NT \d+\.\d+").unwrap();
    result = re_nt
      .replace_all(&result, format!("Windows NT {chosen}").as_str())
      .to_string();
  }

  result
}

/// Add subtle variation to WebGL parameters so that profiles sharing the same
/// base WebGL entry in the database still produce different parameter hashes.
///
/// We only modify parameters that naturally vary between GPU drivers and
/// driver versions:
/// - MAX_TEXTURE_SIZE, MAX_VIEWPORT_DIMS, MAX_RENDERBUFFER_SIZE: power-of-two
///   values that differ across hardware (2048, 4096, 8192, 16384).
/// - ALIASED_LINE_WIDTH_RANGE / ALIASED_POINT_SIZE_RANGE: float ranges that
///   differ per driver.
/// - MAX_COMBINED_TEXTURE_IMAGE_UNITS, MAX_VERTEX_TEXTURE_IMAGE_UNITS: small
///   integer values that vary.
fn diversify_webgl_parameters(
  config: &mut HashMap<String, serde_json::Value>,
  rng: &mut impl rand::Rng,
) {
  use rand::RngExt;

  // Keys for WebGL 1 and WebGL 2 parameters
  for params_key in &["webGl:parameters", "webGl2:parameters"] {
    if let Some(serde_json::Value::Object(params)) = config.get_mut(*params_key) {
      // MAX_TEXTURE_SIZE (param 3379): common values 4096, 8192, 16384
      let max_tex_sizes = [4096i64, 8192, 16384, 16384]; // weighted toward 16384
      let chosen_tex = max_tex_sizes[rng.random_range(0..max_tex_sizes.len())];
      params.insert("3379".to_string(), serde_json::json!(chosen_tex));

      // MAX_RENDERBUFFER_SIZE (param 34024): same range as MAX_TEXTURE_SIZE
      params.insert("34024".to_string(), serde_json::json!(chosen_tex));

      // MAX_VIEWPORT_DIMS (param 3386): pair of [max_tex, max_tex]
      params.insert(
        "3386".to_string(),
        serde_json::json!([chosen_tex, chosen_tex]),
      );

      // MAX_COMBINED_TEXTURE_IMAGE_UNITS (param 35661): 16, 32, 48, 64, 80
      let combined_units = [32i64, 48, 64, 80];
      params.insert(
        "35661".to_string(),
        serde_json::json!(combined_units[rng.random_range(0..combined_units.len())]),
      );

      // MAX_VERTEX_TEXTURE_IMAGE_UNITS (param 35660): 0, 4, 8, 16
      let vert_units = [4i64, 8, 16, 16];
      params.insert(
        "35660".to_string(),
        serde_json::json!(vert_units[rng.random_range(0..vert_units.len())]),
      );

      // MAX_TEXTURE_IMAGE_UNITS (param 34930): 8, 16, 32
      let frag_units = [8i64, 16, 16, 32];
      params.insert(
        "34930".to_string(),
        serde_json::json!(frag_units[rng.random_range(0..frag_units.len())]),
      );

      // ALIASED_LINE_WIDTH_RANGE (param 33902): varies per driver
      let line_max = [1.0f64, 7.375, 10.0, 14.0];
      params.insert(
        "33902".to_string(),
        serde_json::json!([1, line_max[rng.random_range(0..line_max.len())]]),
      );

      // ALIASED_POINT_SIZE_RANGE (param 33901): varies per driver
      let point_max = [255.875f64, 511.0, 1024.0, 8192.0];
      params.insert(
        "33901".to_string(),
        serde_json::json!([1, point_max[rng.random_range(0..point_max.len())]]),
      );

      // MAX_FRAGMENT_UNIFORM_VECTORS (param 36349): 256, 512, 1024, 4096
      let frag_uniforms = [256i64, 512, 1024, 4096];
      params.insert(
        "36349".to_string(),
        serde_json::json!(frag_uniforms[rng.random_range(0..frag_uniforms.len())]),
      );

      // MAX_VERTEX_UNIFORM_VECTORS (param 36347): 256, 512, 1024, 4096
      let vert_uniforms = [256i64, 512, 1024, 4096];
      params.insert(
        "36347".to_string(),
        serde_json::json!(vert_uniforms[rng.random_range(0..vert_uniforms.len())]),
      );

      // MAX_VARYING_VECTORS (param 36348): 8, 15, 16, 30, 31, 32
      let varying = [15i64, 16, 30, 31, 32];
      params.insert(
        "36348".to_string(),
        serde_json::json!(varying[rng.random_range(0..varying.len())]),
      );
    }
  }
}

/// Handle window.screenX and window.screenY generation.
fn handle_screen_xy(config: &mut HashMap<String, serde_json::Value>, screen: &ScreenFingerprint) {
  if config.contains_key("window.screenY") {
    return;
  }

  let screen_x = screen.screen_x;
  if screen_x == 0 {
    config.insert("window.screenX".to_string(), serde_json::json!(0));
    config.insert("window.screenY".to_string(), serde_json::json!(0));
    return;
  }

  if (-50..=50).contains(&screen_x) {
    config.insert("window.screenY".to_string(), serde_json::json!(screen_x));
    return;
  }

  let screen_y = screen.avail_height as i32 - screen.outer_height as i32;
  let mut rng = rand::rng();

  let y = if screen_y == 0 {
    0
  } else if screen_y > 0 {
    rng.random_range(0..=screen_y)
  } else {
    rng.random_range(screen_y..=0)
  };

  config.insert("window.screenY".to_string(), serde_json::json!(y));
}

/// GeoIP option - can be an IP address string or auto-detect.
#[derive(Debug, Clone)]
pub enum GeoIPOption {
  /// Auto-detect IP (fetch public IP, optionally through proxy)
  Auto,
  /// Use a specific IP address
  IP(String),
}

/// Configuration builder for Camoufox launch.
#[derive(Debug, Clone)]
pub struct CamoufoxConfigBuilder {
  fingerprint: Option<Fingerprint>,
  operating_system: Option<String>,
  screen_constraints: Option<ScreenConstraints>,
  block_images: bool,
  block_webrtc: bool,
  block_webgl: bool,
  custom_fonts: Option<Vec<String>>,
  custom_fonts_only: bool,
  firefox_prefs: HashMap<String, serde_json::Value>,
  proxy: Option<ProxyConfig>,
  headless: bool,
  ff_version: Option<u32>,
  extra_config: HashMap<String, serde_json::Value>,
  geoip: Option<GeoIPOption>,
}

/// Proxy configuration.
#[derive(Debug, Clone)]
pub struct ProxyConfig {
  pub server: String,
  pub username: Option<String>,
  pub password: Option<String>,
  pub bypass: Option<String>,
}

impl ProxyConfig {
  /// Parse a proxy URL string into ProxyConfig.
  /// Supports formats like:
  /// - "http://host:port"
  /// - "http://user:pass@host:port"
  /// - "socks5://user:pass@host:port"
  pub fn from_url(url: &str) -> Result<Self, ConfigError> {
    let parsed = url::Url::parse(url).map_err(|e| ConfigError::InvalidProxy(e.to_string()))?;

    let host = parsed
      .host_str()
      .ok_or_else(|| ConfigError::InvalidProxy("Missing host".to_string()))?;

    let port = parsed.port().unwrap_or(8080);
    let scheme = parsed.scheme();

    let server = format!("{scheme}://{host}:{port}");

    let username = if !parsed.username().is_empty() {
      Some(parsed.username().to_string())
    } else {
      None
    };

    let password = parsed.password().map(String::from);

    Ok(Self {
      server,
      username,
      password,
      bypass: None,
    })
  }
}

impl Default for CamoufoxConfigBuilder {
  fn default() -> Self {
    Self::new()
  }
}

impl CamoufoxConfigBuilder {
  pub fn new() -> Self {
    Self {
      fingerprint: None,
      operating_system: None,
      screen_constraints: None,
      block_images: false,
      block_webrtc: false,
      block_webgl: false,
      custom_fonts: None,
      custom_fonts_only: false,
      firefox_prefs: HashMap::new(),
      proxy: None,
      headless: false,
      ff_version: None,
      extra_config: HashMap::new(),
      geoip: None,
    }
  }

  pub fn fingerprint(mut self, fp: Fingerprint) -> Self {
    self.fingerprint = Some(fp);
    self
  }

  pub fn operating_system(mut self, os: &str) -> Self {
    self.operating_system = Some(os.to_string());
    self
  }

  pub fn screen_constraints(mut self, constraints: ScreenConstraints) -> Self {
    self.screen_constraints = Some(constraints);
    self
  }

  pub fn block_images(mut self, block: bool) -> Self {
    self.block_images = block;
    self
  }

  pub fn block_webrtc(mut self, block: bool) -> Self {
    self.block_webrtc = block;
    self
  }

  pub fn block_webgl(mut self, block: bool) -> Self {
    self.block_webgl = block;
    self
  }

  pub fn custom_fonts(mut self, fonts: Vec<String>) -> Self {
    self.custom_fonts = Some(fonts);
    self
  }

  pub fn custom_fonts_only(mut self, only: bool) -> Self {
    self.custom_fonts_only = only;
    self
  }

  pub fn firefox_pref<V: Into<serde_json::Value>>(mut self, key: &str, value: V) -> Self {
    self.firefox_prefs.insert(key.to_string(), value.into());
    self
  }

  pub fn proxy(mut self, proxy: ProxyConfig) -> Self {
    self.proxy = Some(proxy);
    self
  }

  pub fn headless(mut self, headless: bool) -> Self {
    self.headless = headless;
    self
  }

  pub fn ff_version(mut self, version: u32) -> Self {
    self.ff_version = Some(version);
    self
  }

  pub fn extra_config<V: Into<serde_json::Value>>(mut self, key: &str, value: V) -> Self {
    self.extra_config.insert(key.to_string(), value.into());
    self
  }

  /// Set GeoIP option for geolocation-based fingerprinting.
  /// Use `GeoIPOption::Auto` to auto-detect public IP (optionally through proxy).
  /// Use `GeoIPOption::IP(ip_string)` to use a specific IP address.
  pub fn geoip(mut self, option: GeoIPOption) -> Self {
    self.geoip = Some(option);
    self
  }

  /// Build the complete Camoufox launch configuration.
  pub fn build(self) -> Result<CamoufoxLaunchConfig, ConfigError> {
    // Generate or use provided fingerprint
    let fingerprint = if let Some(fp) = self.fingerprint {
      fp
    } else {
      // Use fpgen (trained on 2.4M real sessions) for realistic fingerprints
      let os_name = self.operating_system.as_deref().unwrap_or("windows");
      let fpgen_platform = match os_name {
        "macos" => "mac",
        other => other,
      };

      let fpgen_result = crate::fpgen::generate(Some(&crate::fpgen::FpgenOptions {
        browser: Some("Firefox".into()),
        platform: Some(fpgen_platform.into()),
      }));

      match fpgen_result {
        Ok(fpgen_json) => {
          let options = FingerprintOptions {
            operating_system: self.operating_system.clone(),
            browsers: Some(vec!["firefox".to_string()]),
            devices: Some(vec!["desktop".to_string()]),
            screen: self.screen_constraints.clone(),
            ..Default::default()
          };
          match crate::fpgen::convert::to_fingerprint(&fpgen_json, &options) {
            Ok(result) => {
              log::info!("fpgen: generated fingerprint for {}", os_name);
              result.fingerprint
            }
            Err(e) => {
              log::warn!(
                "fpgen conversion failed ({}), falling back to BrowserForge",
                e
              );
              self.fallback_browserforge_fingerprint()?
            }
          }
        }
        Err(e) => {
          log::warn!(
            "fpgen generation failed ({}), falling back to BrowserForge",
            e
          );
          self.fallback_browserforge_fingerprint()?
        }
      }
    };

    // Determine target OS from user agent
    let target_os = env_vars::determine_ua_os(&fingerprint.navigator.user_agent);

    // Convert fingerprint to config
    let mut config = from_browserforge(&fingerprint, self.ff_version);

    let mut rng = rand::rng();

    // -------------------------------------------------------------------
    // User Agent diversification.
    //
    // fpgen's Bayesian network for Firefox on a specific OS often returns
    // the same UA string because the probability distribution is dominated
    // by the latest Firefox release.  To ensure every profile has a unique
    // UA (the single most impactful fingerprint signal), we randomize the
    // Firefox version within a realistic recent range and, for macOS, also
    // vary the macOS version string.
    // -------------------------------------------------------------------
    if let Some(ua_val) = config.get("navigator.userAgent").cloned() {
      if let Some(ua) = ua_val.as_str() {
        let diversified = diversify_user_agent(ua, self.ff_version, &mut rng);
        if diversified != ua {
          config.insert(
            "navigator.userAgent".to_string(),
            serde_json::json!(diversified),
          );
          // Also update appVersion to match (Firefox uses "5.0 (platform)")
          // appVersion doesn't include the rv, so we leave it as-is.
        }

        // Keep navigator.oscpu consistent with the (possibly diversified) UA.
        // Firefox's oscpu reflects the OS version from the UA string.
        // We always update oscpu from the final UA, regardless of whether
        // diversification changed anything, to ensure consistency.
        let final_ua = if diversified != ua { &diversified } else { ua };
        if final_ua.contains("Mac OS X") {
          let re_mac = regex_lite::Regex::new(r"Mac OS X (\d+\.\d+)").unwrap();
          if let Some(caps) = re_mac.captures(final_ua) {
            let mac_ver = caps.get(1).unwrap().as_str();
            // Update oscpu to match — may be missing, null, or have old version
            let new_oscpu = format!("Intel Mac OS X {mac_ver}");
            config.insert("navigator.oscpu".to_string(), serde_json::json!(new_oscpu));
          }
        } else if final_ua.contains("Windows NT") {
          let re_nt = regex_lite::Regex::new(r"Windows NT (\d+\.\d+)").unwrap();
          if let Some(caps) = re_nt.captures(final_ua) {
            let nt_ver = caps.get(1).unwrap().as_str();
            let new_oscpu = if nt_ver == "10.0" {
              "Windows NT 10.0; Win64; x64".to_string()
            } else {
              format!("Windows NT {nt_ver}; Win64; x64")
            };
            config.insert("navigator.oscpu".to_string(), serde_json::json!(new_oscpu));
          }
        }
      }
    }

    // Add random window history length
    config.insert(
      "window.history.length".to_string(),
      serde_json::json!(rng.random_range(1..=5)),
    );

    // -------------------------------------------------------------------
    // Window inner/outer dimensions: ensure per-profile variation.
    //
    // The browserforge mapping copies screen.innerWidth → window.innerWidth
    // etc., but fpgen's Bayesian network for Firefox often omits these
    // fields, so they collapse to identical defaults.  We re-derive them
    // from the actual screen dimensions with random chrome offsets so that
    // every profile gets unique window dimensions.
    // -------------------------------------------------------------------
    let screen_w = config
      .get("screen.width")
      .and_then(|v| v.as_u64())
      .unwrap_or(1920) as u32;
    let avail_h = config
      .get("screen.availHeight")
      .and_then(|v| v.as_u64())
      .unwrap_or(1040) as u32;

    // Firefox chrome (tabs + toolbars + title bar) varies 71-111 px
    let chrome_h: u32 = rng.random_range(71..=111);
    // 40% chance window is maximized width, otherwise slightly narrower
    let w_offset: u32 = if rng.random_range(0u32..=99) < 40 {
      0
    } else {
      rng.random_range(0..=120)
    };
    let ow = screen_w.saturating_sub(w_offset);
    let oh = avail_h;
    let iw = ow;
    let ih = oh.saturating_sub(chrome_h);

    config.insert("window.innerWidth".to_string(), serde_json::json!(iw));
    config.insert("window.innerHeight".to_string(), serde_json::json!(ih));
    config.insert("window.outerWidth".to_string(), serde_json::json!(ow));
    config.insert("window.outerHeight".to_string(), serde_json::json!(oh));

    // -------------------------------------------------------------------
    // window.screenX / screenY: add per-profile variation.
    //
    // The handle_screen_xy function often sets both to 0 (especially on
    // macOS where screen_x from fpgen is typically 0). We override with
    // realistic random values: most users have the window near (0,0) but
    // some have non-zero values from dragging the window.
    // -------------------------------------------------------------------
    {
      let sx = if rng.random_range(0u32..=99) < 30 {
        rng.random_range(0..=200) as i32 // 30% chance of offset
      } else {
        0
      };
      let sy = if sx > 0 {
        rng.random_range(0..=100) as i32
      } else if rng.random_range(0u32..=99) < 20 {
        rng.random_range(0..=80) as i32 // 20% chance of small Y offset even with X=0
      } else {
        0
      };
      config.insert("window.screenX".to_string(), serde_json::json!(sx));
      config.insert("window.screenY".to_string(), serde_json::json!(sy));
    }

    // -------------------------------------------------------------------
    // screen.colorDepth / pixelDepth: add per-profile variation.
    //
    // macOS commonly reports 24 or 30 (for HDR/wide-gamut displays).
    // Windows usually reports 24. By randomizing between realistic values,
    // different profiles get different color depth fingerprints.
    // -------------------------------------------------------------------
    {
      let color_depth: u32 = match target_os {
        "mac" | "macos" => {
          // macOS: 24 (standard) or 30 (P3 wide-gamut / HDR)
          if rng.random_range(0u32..=99) < 50 {
            30
          } else {
            24
          }
        }
        _ => 24, // Windows/Linux are almost always 24
      };
      config.insert(
        "screen.colorDepth".to_string(),
        serde_json::json!(color_depth),
      );
      config.insert(
        "screen.pixelDepth".to_string(),
        serde_json::json!(color_depth),
      );
    }

    // -------------------------------------------------------------------
    // Fonts: add per-profile variation.
    //
    // The OS font list is deterministic, which means every profile for
    // the same OS has an identical font fingerprint.  We randomly drop
    // a small subset of "optional" fonts (keeping core fonts that every
    // real machine would have) so that different profiles expose slightly
    // different font lists — enough to produce a different font hash.
    // -------------------------------------------------------------------
    if !self.custom_fonts_only {
      let system_fonts = fonts::get_fonts_for_os(target_os);
      let mut font_list = if let Some(custom) = &self.custom_fonts {
        let mut all_fonts = system_fonts;
        for font in custom {
          if !all_fonts.contains(font) {
            all_fonts.push(font.clone());
          }
        }
        all_fonts
      } else {
        system_fonts
      };

      // Core fonts that must always be present (common baseline fonts).
      let core_fonts: std::collections::HashSet<&str> = [
        "Arial",
        "Helvetica",
        "Times New Roman",
        "Courier New",
        "Verdana",
        "Georgia",
        "Trebuchet MS",
        "Comic Sans MS",
        "Helvetica Neue",
        "Lucida Grande",
        "Menlo",
        "Monaco",
      ]
      .into_iter()
      .collect();

      // Randomly drop 5-15% of non-core fonts for per-profile variation.
      let drop_pct = rng.random_range(5u32..=15);
      font_list.retain(|f| {
        if core_fonts.contains(f.as_str()) {
          true // always keep core fonts
        } else {
          rng.random_range(0u32..=99) >= drop_pct
        }
      });

      config.insert("fonts".to_string(), serde_json::json!(font_list));
    } else if let Some(custom) = &self.custom_fonts {
      config.insert("fonts".to_string(), serde_json::json!(custom));
    }

    // Font spacing seed — deterministic per-profile value for unique font metrics
    config.insert(
      "fonts:spacing_seed".to_string(),
      serde_json::json!(rng.random_range(0..1_073_741_824u32)),
    );

    // Build Firefox preferences
    let mut firefox_prefs = self.firefox_prefs;

    // Override Playwright-inherited prefs that cause Browser Tampering detection.
    // These must match vanilla Firefox defaults to avoid fingerprint.com anomaly detection.
    firefox_prefs.insert(
      "ui.use_standins_for_native_colors".to_string(),
      serde_json::json!(false),
    );
    firefox_prefs.insert(
      "gfx.color_management.mode".to_string(),
      serde_json::json!(2),
    );
    firefox_prefs.insert(
      "gfx.color_management.rendering_intent".to_string(),
      serde_json::json!(0),
    );
    firefox_prefs.insert(
      "focusmanager.testmode".to_string(),
      serde_json::json!(false),
    );
    firefox_prefs.insert(
      "toolkit.cosmeticAnimations.enabled".to_string(),
      serde_json::json!(true),
    );

    if self.block_images {
      firefox_prefs.insert(
        "permissions.default.image".to_string(),
        serde_json::json!(2),
      );
    }

    if self.block_webrtc {
      firefox_prefs.insert(
        "media.peerconnection.enabled".to_string(),
        serde_json::json!(false),
      );
    }

    if self.block_webgl {
      firefox_prefs.insert("webgl.disabled".to_string(), serde_json::json!(true));
    } else {
      // Sample and add WebGL configuration.
      // Each call to sample_webgl uses its own RNG so different profiles
      // can get different vendor/renderer/parameter combinations.
      match webgl::sample_webgl(target_os, None, None) {
        Ok(webgl_data) => {
          for (key, value) in webgl_data.config {
            config.insert(key, value);
          }
          // webgl.sanitize-unmasked-renderer MUST be false when Camoufox spoofs
          // WebGL vendor/renderer — otherwise WebGL1 exposes the spoofed renderer
          // but WebGL2 doesn't (WEBGL_debug_renderer_info blocked), creating an
          // inconsistency that fingerprint.com detects as anomaly_score=1.
          firefox_prefs.insert(
            "webgl.sanitize-unmasked-renderer".to_string(),
            serde_json::json!(false),
          );

          // ---------------------------------------------------------------
          // WebGL parameter diversification.
          //
          // The WebGL database for a given OS often has limited unique
          // entries, so parameters/shaderPrecisionFormats/contextAttributes
          // end up identical across profiles.  We add subtle, realistic
          // variation to certain numeric parameters that naturally differ
          // between GPU drivers, without changing values that must be
          // exact powers of two or specific constants.
          // ---------------------------------------------------------------
          diversify_webgl_parameters(&mut config, &mut rng);
        }
        Err(e) => {
          log::warn!("Failed to sample WebGL config: {}", e);
        }
      }
    }

    // Canvas anti-aliasing offset — uses a deterministic per-profile value
    // to produce unique but consistent canvas hashes per profile.
    // The offset is derived from the same RNG seed as other fingerprint values,
    // ensuring each profile gets a different canvas hash while remaining
    // consistent across page loads within the same session.
    let aa_offset = rng.random_range(-50..=50);
    config.insert("canvas:aaOffset".to_string(), serde_json::json!(aa_offset));
    config.insert("canvas:aaCapOffset".to_string(), serde_json::json!(true));

    // -------------------------------------------------------------------
    // screenX / screenY: add per-profile variation.
    //
    // Most profiles get screenX=screenY=0 (window at top-left). Real users
    // often have windows offset from (0,0). Add a small random offset.
    // -------------------------------------------------------------------
    {
      // 60% chance of (0,0), 40% chance of a random small offset
      if rng.random_range(0u32..=99) >= 60 {
        let sx = rng.random_range(0i32..=200);
        let sy = rng.random_range(0i32..=100);
        config.insert("window.screenX".to_string(), serde_json::json!(sx));
        config.insert("window.screenY".to_string(), serde_json::json!(sy));
      }
    }

    // -------------------------------------------------------------------
    // WebGL extension list perturbation for per-profile uniqueness.
    //
    // When multiple profiles sample the same GPU (e.g. "Apple M1" on macOS),
    // the extension lists and shader precision formats are identical, making
    // the WebGL hash a shared signal. We randomly remove 1-3 optional
    // extensions from the list to produce a unique WebGL hash per profile.
    // -------------------------------------------------------------------
    for ext_key in &["webGl:supportedExtensions", "webGl2:supportedExtensions"] {
      if let Some(val) = config.get(*ext_key).cloned() {
        if let Some(arr) = val.as_array() {
          let mut exts: Vec<serde_json::Value> = arr.clone();
          // Only perturb if there are enough extensions to safely remove some
          if exts.len() > 10 {
            let num_to_drop = rng.random_range(1u32..=3) as usize;
            for _ in 0..num_to_drop {
              if exts.len() > 5 {
                let idx = rng.random_range(0..exts.len());
                exts.remove(idx);
              }
            }
            config.insert(ext_key.to_string(), serde_json::json!(exts));
          }
        }
      }
    }

    // Add extra config (user-provided)
    for (key, value) in self.extra_config {
      config.insert(key, value);
    }

    // Keep theming enabled — disabling it causes abnormal CSS media query
    // responses (prefers-color-scheme, system colors) which triggers
    // Browser Tampering detection on fingerprint.com
    // config.insert("disableTheming".to_string(), serde_json::json!(true));

    // Show cursor in headed mode (default behavior)
    config.insert("showcursor".to_string(), serde_json::json!(true));

    // Fix navigator.plugins — Camoufox defaults to 5 PDF viewers (including
    // Chrome/Edge/WebKit ones) which is impossible for Firefox and causes
    // anomaly_score=1 on fingerprint.com. Real Firefox only has "PDF Viewer".
    config.insert(
      "navigator.plugins".to_string(),
      serde_json::json!([{
        "name": "PDF Viewer",
        "filename": "internal-pdf-viewer",
        "description": "Portable Document Format",
        "mimeTypes": [
          {"type": "application/pdf", "suffixes": "pdf", "description": "Portable Document Format"},
          {"type": "text/pdf", "suffixes": "pdf", "description": "Portable Document Format"}
        ]
      }]),
    );

    Ok(CamoufoxLaunchConfig {
      fingerprint_config: config,
      firefox_prefs,
      proxy: self.proxy,
      headless: self.headless,
      target_os: target_os.to_string(),
    })
  }

  /// Build the complete Camoufox launch configuration with async geolocation support.
  /// This method should be used when geoip option is set to Auto.
  pub async fn build_async(self) -> Result<CamoufoxLaunchConfig, ConfigError> {
    // Get full proxy URL (with credentials) for IP detection
    let proxy_url = self.proxy.as_ref().map(|p| {
      if let (Some(user), Some(pass)) = (&p.username, &p.password) {
        // Reconstruct URL with credentials: scheme://user:pass@host:port
        if let Ok(mut parsed) = url::Url::parse(&p.server) {
          let _ = parsed.set_username(user);
          let _ = parsed.set_password(Some(pass));
          parsed.to_string()
        } else {
          p.server.clone()
        }
      } else if let Some(user) = &p.username {
        if let Ok(mut parsed) = url::Url::parse(&p.server) {
          let _ = parsed.set_username(user);
          parsed.to_string()
        } else {
          p.server.clone()
        }
      } else {
        p.server.clone()
      }
    });
    let geoip_option = self.geoip.clone();
    let block_webrtc = self.block_webrtc;

    // Build base config first
    let mut launch_config = self.build()?;

    // Handle geolocation if geoip option is set
    if let Some(geoip) = geoip_option {
      let ip = match geoip {
        GeoIPOption::Auto => {
          // Fetch public IP, optionally through proxy
          geolocation::fetch_public_ip(proxy_url.as_deref())
            .await
            .map_err(geolocation::GeolocationError::from)?
        }
        GeoIPOption::IP(ip_str) => {
          if !geolocation::validate_ip(&ip_str) {
            return Err(ConfigError::Geolocation(
              geolocation::GeolocationError::InvalidIP(ip_str),
            ));
          }
          ip_str
        }
      };

      // Get geolocation from IP
      match geolocation::get_geolocation(&ip) {
        Ok(geo) => {
          // Add geolocation config
          for (key, value) in geo.as_config() {
            launch_config.fingerprint_config.insert(key, value);
          }

          // Add WebRTC IP spoofing if not blocked
          if !block_webrtc {
            if geolocation::is_ipv4(&ip) {
              launch_config
                .fingerprint_config
                .insert("webrtc:ipv4".to_string(), serde_json::json!(ip));
            } else if geolocation::is_ipv6(&ip) {
              launch_config
                .fingerprint_config
                .insert("webrtc:ipv6".to_string(), serde_json::json!(ip));
            }
          }

          log::info!(
            "Applied geolocation from IP {}: {} ({})",
            ip,
            geo.locale.as_string(),
            geo.timezone
          );
        }
        Err(e) => {
          log::warn!("Failed to get geolocation for IP {}: {}", ip, e);
          // Continue without geolocation rather than failing
        }
      }
    }

    Ok(launch_config)
  }

  /// Fallback: generate fingerprint using the old BrowserForge Bayesian networks.
  fn fallback_browserforge_fingerprint(&self) -> Result<Fingerprint, ConfigError> {
    let generator = crate::camoufox::fingerprint::FingerprintGenerator::new()?;
    let options = FingerprintOptions {
      operating_system: self.operating_system.clone(),
      browsers: Some(vec!["firefox".to_string()]),
      devices: Some(vec!["desktop".to_string()]),
      screen: self.screen_constraints.clone(),
      ..Default::default()
    };
    Ok(generator.get_fingerprint(&options)?.fingerprint)
  }
}

/// Complete Camoufox launch configuration.
#[derive(Debug, Clone)]
pub struct CamoufoxLaunchConfig {
  pub fingerprint_config: HashMap<String, serde_json::Value>,
  pub firefox_prefs: HashMap<String, serde_json::Value>,
  pub proxy: Option<ProxyConfig>,
  pub headless: bool,
  pub target_os: String,
}

impl CamoufoxLaunchConfig {
  /// Get environment variables for launching Camoufox.
  pub fn get_env_vars(&self) -> Result<HashMap<String, String>, serde_json::Error> {
    env_vars::config_to_env_vars(&self.fingerprint_config)
  }

  /// Get the config as JSON string.
  pub fn config_json(&self) -> Result<String, serde_json::Error> {
    serde_json::to_string(&self.fingerprint_config)
  }
}

/// Error type for configuration operations.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
  #[error("Fingerprint generation error: {0}")]
  Fingerprint(#[from] crate::camoufox::fingerprint::FingerprintError),

  #[error("JSON error: {0}")]
  Json(#[from] serde_json::Error),

  #[error("WebGL error: {0}")]
  WebGL(#[from] webgl::WebGLError),

  #[error("Invalid proxy configuration: {0}")]
  InvalidProxy(String),

  #[error("Geolocation error: {0}")]
  Geolocation(#[from] crate::camoufox::geolocation::GeolocationError),
}

/// Get Firefox version from executable path.
pub fn get_firefox_version(executable_path: &Path) -> Option<u32> {
  // Try to read version.json from the same directory
  let version_path = executable_path.parent()?.join("version.json");

  if let Ok(content) = std::fs::read_to_string(&version_path) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
      if let Some(version_str) = json.get("version").and_then(|v| v.as_str()) {
        // Parse major version from "135.0" or similar
        let major: u32 = version_str.split('.').next()?.parse().ok()?;
        return Some(major);
      }
    }
  }

  None
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_config_builder() {
    let config = CamoufoxConfigBuilder::new()
      .operating_system("windows")
      .block_images(true)
      .build();

    assert!(config.is_ok());
    let config = config.unwrap();
    assert!(config
      .firefox_prefs
      .contains_key("permissions.default.image"));
  }

  #[test]
  fn test_replace_ff_version() {
    let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0";
    let replaced = replace_ff_version(ua, 140);
    assert!(replaced.contains("140.0"));
  }

  #[test]
  fn test_fingerprint_uniqueness() {
    // Generate 5 fingerprint configs and compare them
    let num_profiles = 5;
    let mut configs: Vec<HashMap<String, serde_json::Value>> = Vec::new();

    for i in 0..num_profiles {
      let config = CamoufoxConfigBuilder::new()
        .operating_system("macos")
        .build()
        .unwrap_or_else(|e| panic!("Failed to build config {i}: {e}"));
      configs.push(config.fingerprint_config);
    }

    // Track which keys differ across profiles
    let all_keys: std::collections::HashSet<String> =
      configs.iter().flat_map(|c| c.keys().cloned()).collect();

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

    eprintln!("\n=== FINGERPRINT UNIQUENESS TEST ({num_profiles} profiles, macOS) ===");
    eprintln!("Total keys: {}", all_keys.len());
    eprintln!(
      "Identical across all profiles: {} ({:.0}%)",
      identical_keys.len(),
      100.0 * identical_keys.len() as f64 / all_keys.len() as f64
    );
    eprintln!(
      "Different across profiles: {} ({:.0}%)",
      different_keys.len(),
      100.0 * different_keys.len() as f64 / all_keys.len() as f64
    );

    eprintln!("\n--- DIFFERENT KEYS ---");
    for key in &different_keys {
      let values: Vec<String> = configs
        .iter()
        .map(|c| {
          c.get(key)
            .map(|v| {
              let s = v.to_string();
              if s.len() > 60 {
                format!("{}...", &s[..60])
              } else {
                s
              }
            })
            .unwrap_or_else(|| "MISSING".to_string())
        })
        .collect();
      eprintln!("  {key}:");
      for (i, val) in values.iter().enumerate() {
        eprintln!("    profile {i}: {val}");
      }
    }

    eprintln!("\n--- IDENTICAL KEYS ---");
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
      eprintln!("  {key} = {val}");
    }

    // Key signals that fingerprint.com uses - these MUST differ
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

    eprintln!("\n--- CRITICAL SIGNAL CHECK ---");
    let mut critical_different = 0;
    for key in &critical_keys {
      let values: Vec<Option<&serde_json::Value>> = configs.iter().map(|c| c.get(*key)).collect();
      let first = values[0];
      let all_same = values.iter().all(|v| v == &first);
      let status = if all_same {
        "IDENTICAL (BAD)"
      } else {
        "DIFFERENT (GOOD)"
      };
      if !all_same {
        critical_different += 1;
      }
      eprintln!("  {key}: {status}");
    }

    // At least 50% of critical keys should differ across profiles
    let min_different = critical_keys.len() / 2;
    assert!(
      critical_different >= min_different,
      "Only {critical_different}/{} critical fingerprint signals differ across profiles. Need at least {min_different} different.",
      critical_keys.len()
    );

    // Overall: less than 60% of keys should be identical
    let identical_pct = 100.0 * identical_keys.len() as f64 / all_keys.len() as f64;
    assert!(
      identical_pct < 60.0,
      "{}% of fingerprint config keys are identical across profiles (max 60%)",
      identical_pct as u32
    );
  }

  #[test]
  fn test_from_browserforge() {
    let fingerprint = Fingerprint {
      screen: ScreenFingerprint {
        width: 1920,
        height: 1080,
        avail_width: 1920,
        avail_height: 1040,
        color_depth: 24,
        pixel_depth: 24,
        inner_width: 1903,
        inner_height: 969,
        outer_width: 1920,
        outer_height: 1040,
        ..Default::default()
      },
      navigator: NavigatorFingerprint {
        user_agent: "Mozilla/5.0 Firefox/135.0".to_string(),
        platform: "Win32".to_string(),
        language: "en-US".to_string(),
        languages: vec!["en-US".to_string()],
        hardware_concurrency: 8,
        ..Default::default()
      },
      ..Default::default()
    };

    let config = from_browserforge(&fingerprint, Some(140));

    assert!(config.contains_key("navigator.userAgent"));
    assert!(config.contains_key("screen.width"));
  }
}
