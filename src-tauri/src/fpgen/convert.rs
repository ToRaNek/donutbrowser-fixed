//! Convert fpgen JSON output to DonutBrowser's `Fingerprint` type.

use crate::camoufox::fingerprint::types::*;
use std::collections::HashMap;

/// Convert an fpgen-generated JSON fingerprint into the DonutBrowser `Fingerprint` struct.
pub fn to_fingerprint(
  fpgen_json: &serde_json::Value,
  options: &FingerprintOptions,
) -> Result<FingerprintWithHeaders, String> {
  let obj = fpgen_json
    .as_object()
    .ok_or("fpgen output is not a JSON object")?;

  // --- Screen ---
  let screen_obj = obj.get("screen").and_then(|v| v.as_object());
  let screen = ScreenFingerprint {
    width: get_u32(screen_obj, "width", 1920),
    height: get_u32(screen_obj, "height", 1080),
    avail_width: get_u32(screen_obj, "availWidth", 1920),
    avail_height: get_u32(screen_obj, "availHeight", 1040),
    avail_top: get_u32(screen_obj, "availTop", 0),
    avail_left: get_u32(screen_obj, "availLeft", 0),
    color_depth: get_u32(screen_obj, "colorDepth", 24),
    pixel_depth: get_u32(screen_obj, "pixelDepth", 24),
    device_pixel_ratio: screen_obj
      .and_then(|s| s.get("devicePixelRatio"))
      .and_then(|v| v.as_f64())
      .unwrap_or(1.0),
    inner_width: get_u32(screen_obj, "innerWidth", 1920),
    inner_height: get_u32(screen_obj, "innerHeight", 969),
    outer_width: get_u32(screen_obj, "outerWidth", 1920),
    outer_height: get_u32(screen_obj, "outerHeight", 1040),
    ..Default::default()
  };

  // --- Navigator ---
  let nav_obj = obj.get("navigator").and_then(|v| v.as_object());

  let user_agent = nav_obj
    .and_then(|n| n.get("userAgent"))
    .and_then(|v| v.as_str())
    .unwrap_or_default()
    .to_string();

  let platform = nav_obj
    .and_then(|n| n.get("platform"))
    .and_then(|v| v.as_str())
    .unwrap_or_default()
    .to_string();

  // Languages from fpgen
  let languages_val = obj.get("languages");
  let languages: Vec<String> = languages_val
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
    })
    .unwrap_or_else(|| vec!["en-US".to_string()]);

  let language = languages
    .first()
    .cloned()
    .unwrap_or_else(|| "en-US".to_string());

  let navigator = NavigatorFingerprint {
    user_agent,
    user_agent_data: nav_obj
      .and_then(|n| n.get("userAgentData"))
      .and_then(|v| serde_json::from_value(v.clone()).ok()),
    do_not_track: nav_obj
      .and_then(|n| n.get("doNotTrack"))
      .and_then(|v| v.as_str().map(String::from)),
    app_code_name: "Mozilla".to_string(),
    app_name: "Netscape".to_string(),
    app_version: nav_obj
      .and_then(|n| n.get("appVersion"))
      .and_then(|v| v.as_str())
      .unwrap_or_default()
      .to_string(),
    oscpu: nav_obj
      .and_then(|n| n.get("oscpu"))
      .and_then(|v| v.as_str().map(String::from)),
    webdriver: None,
    language,
    languages,
    platform,
    device_memory: obj
      .get("memory")
      .and_then(|v| v.as_object())
      .and_then(|m| m.get("deviceMemory"))
      .and_then(|v| v.as_u64())
      .map(|v| v as u32),
    hardware_concurrency: nav_obj
      .and_then(|n| n.get("hardwareConcurrency"))
      .and_then(|v| v.as_u64())
      .unwrap_or(4) as u32,
    product: "Gecko".to_string(),
    product_sub: nav_obj
      .and_then(|n| n.get("productSub"))
      .and_then(|v| v.as_str())
      .unwrap_or("20030107")
      .to_string(),
    vendor: nav_obj
      .and_then(|n| n.get("vendor"))
      .and_then(|v| v.as_str())
      .unwrap_or("Google Inc.")
      .to_string(),
    vendor_sub: String::new(),
    max_touch_points: nav_obj
      .and_then(|n| n.get("maxTouchPoints"))
      .and_then(|v| v.as_u64())
      .unwrap_or(0) as u32,
    extra_properties: None,
  };

  // --- GPU / Video Card ---
  let gpu_obj = obj.get("gpu").and_then(|v| v.as_object());
  let video_card = VideoCard {
    vendor: gpu_obj
      .and_then(|g| g.get("vendor"))
      .and_then(|v| v.as_str())
      .unwrap_or_default()
      .to_string(),
    renderer: gpu_obj
      .and_then(|g| g.get("renderer"))
      .and_then(|v| v.as_str())
      .unwrap_or_default()
      .to_string(),
  };

  // --- Fonts ---
  let fonts: Vec<String> = obj
    .get("allFonts")
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
    })
    .unwrap_or_default();

  // --- Audio/Video codecs ---
  let audio_codecs = extract_codecs(obj, "audio");
  let video_codecs = extract_codecs(obj, "video");

  // --- Plugins ---
  let plugins_data: HashMap<String, String> = obj
    .get("plugins")
    .and_then(|v| serde_json::from_value(v.clone()).ok())
    .unwrap_or_default();

  // --- Battery ---
  let battery = obj
    .get("battery")
    .and_then(|v| serde_json::from_value(v.clone()).ok());

  // --- Multimedia devices ---
  let multimedia_devices: Vec<String> = obj
    .get("mediaDevices")
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
    })
    .unwrap_or_default();

  let fingerprint = Fingerprint {
    screen,
    navigator,
    video_codecs,
    audio_codecs,
    plugins_data,
    battery,
    video_card,
    multimedia_devices,
    fonts,
    mock_web_rtc: options.mock_web_rtc,
    slim: options.slim,
  };

  // --- Headers ---
  let headers: Headers = obj
    .get("headers")
    .and_then(|v| v.as_object())
    .map(|h| {
      h.iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect()
    })
    .unwrap_or_default();

  Ok(FingerprintWithHeaders {
    fingerprint,
    headers,
  })
}

fn get_u32(
  obj: Option<&serde_json::Map<String, serde_json::Value>>,
  key: &str,
  default: u32,
) -> u32 {
  obj
    .and_then(|o| o.get(key))
    .and_then(|v| v.as_u64())
    .unwrap_or(default as u64) as u32
}

fn extract_codecs(
  obj: &serde_json::Map<String, serde_json::Value>,
  _kind: &str,
) -> HashMap<String, String> {
  // fpgen stores codec support in mediaDecoderSupport / encryptedMediaCapabilities
  // For now, return empty — Camoufox handles codec spoofing at C++ level
  let _ = obj;
  HashMap::new()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_convert_fpgen_to_fingerprint() {
    let fpgen_json = crate::fpgen::generate(Some(&crate::fpgen::FpgenOptions {
      browser: Some("Chrome".into()),
      platform: Some("mac".into()),
    }))
    .unwrap();

    let options = FingerprintOptions::default();
    let result = to_fingerprint(&fpgen_json, &options);
    assert!(result.is_ok(), "Conversion failed: {:?}", result.err());

    let fp = result.unwrap();
    assert!(!fp.fingerprint.navigator.user_agent.is_empty());
    assert!(fp.fingerprint.screen.width > 0);
    assert!(fp.fingerprint.screen.height > 0);
    assert!(!fp.fingerprint.video_card.vendor.is_empty());
    assert!(!fp.fingerprint.fonts.is_empty());
    eprintln!("UA: {}", fp.fingerprint.navigator.user_agent);
    eprintln!("Platform: {}", fp.fingerprint.navigator.platform);
    eprintln!(
      "GPU: {} / {}",
      fp.fingerprint.video_card.vendor, fp.fingerprint.video_card.renderer
    );
    eprintln!(
      "Screen: {}x{}",
      fp.fingerprint.screen.width, fp.fingerprint.screen.height
    );
    eprintln!("Fonts: {} fonts", fp.fingerprint.fonts.len());
  }
}
