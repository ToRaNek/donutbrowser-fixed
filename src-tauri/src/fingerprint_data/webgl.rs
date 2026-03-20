/// Per-OS WebGL vendor/renderer databases for cross-OS anti-detect browser
/// profiles.
///
/// Real browsers expose the GPU vendor and renderer strings through the
/// `WEBGL_debug_renderer_info` extension.  When spoofing a different OS the
/// reported values must be plausible for that platform.
#[derive(Debug, Clone)]
pub struct WebGLProfile {
  pub vendor: &'static str,
  pub renderer: &'static str,
}

pub fn get_webgl_profiles_for_os(os: &str) -> Vec<WebGLProfile> {
  match os {
    "windows" => windows_webgl_profiles(),
    "macos" => macos_webgl_profiles(),
    "linux" => linux_webgl_profiles(),
    _ => windows_webgl_profiles(),
  }
}

pub fn get_random_webgl_profile(os: &str, seed: u32) -> WebGLProfile {
  let profiles = get_webgl_profiles_for_os(os);
  let index = (seed as usize) % profiles.len();
  profiles[index].clone()
}

fn windows_webgl_profiles() -> Vec<WebGLProfile> {
  vec![
    WebGLProfile {
      vendor: "Google Inc. (NVIDIA)",
      renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)",
    },
    WebGLProfile {
      vendor: "Google Inc. (NVIDIA)",
      renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 Direct3D11 vs_5_0 ps_5_0, D3D11)",
    },
    WebGLProfile {
      vendor: "Google Inc. (NVIDIA)",
      renderer: "ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 SUPER Direct3D11 vs_5_0 ps_5_0, D3D11)",
    },
    WebGLProfile {
      vendor: "Google Inc. (Intel)",
      renderer: "ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0, D3D11)",
    },
    WebGLProfile {
      vendor: "Google Inc. (Intel)",
      renderer: "ANGLE (Intel, Intel(R) UHD Graphics 770 Direct3D11 vs_5_0 ps_5_0, D3D11)",
    },
    WebGLProfile {
      vendor: "Google Inc. (Intel)",
      renderer: "ANGLE (Intel, Intel(R) Iris(R) Xe Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)",
    },
    WebGLProfile {
      vendor: "Google Inc. (AMD)",
      renderer: "ANGLE (AMD, AMD Radeon RX 6700 XT Direct3D11 vs_5_0 ps_5_0, D3D11)",
    },
    WebGLProfile {
      vendor: "Google Inc. (AMD)",
      renderer: "ANGLE (AMD, AMD Radeon RX 580 Direct3D11 vs_5_0 ps_5_0, D3D11)",
    },
  ]
}

fn macos_webgl_profiles() -> Vec<WebGLProfile> {
  vec![
    WebGLProfile {
      vendor: "Apple",
      renderer: "Apple M1",
    },
    WebGLProfile {
      vendor: "Apple",
      renderer: "Apple M1 Pro",
    },
    WebGLProfile {
      vendor: "Apple",
      renderer: "Apple M2",
    },
    WebGLProfile {
      vendor: "Apple",
      renderer: "Apple M2 Pro",
    },
    WebGLProfile {
      vendor: "Apple",
      renderer: "Apple M3",
    },
    WebGLProfile {
      vendor: "Apple",
      renderer: "Apple M3 Pro",
    },
    WebGLProfile {
      vendor: "Apple",
      renderer: "Intel(R) Iris(TM) Plus Graphics 655",
    },
    WebGLProfile {
      vendor: "Apple",
      renderer: "Intel(R) UHD Graphics 630",
    },
    WebGLProfile {
      vendor: "Apple",
      renderer: "AMD Radeon Pro 5500M",
    },
  ]
}

fn linux_webgl_profiles() -> Vec<WebGLProfile> {
  vec![
    WebGLProfile {
      vendor: "Mesa",
      renderer: "Mesa Intel(R) UHD Graphics 630 (CFL GT2)",
    },
    WebGLProfile {
      vendor: "Mesa",
      renderer: "Mesa Intel(R) UHD Graphics 770 (ADL-S GT1)",
    },
    WebGLProfile {
      vendor: "X.Org",
      renderer: "NVIDIA GeForce GTX 1080/PCIe/SSE2",
    },
    WebGLProfile {
      vendor: "X.Org",
      renderer: "NVIDIA GeForce RTX 3060/PCIe/SSE2",
    },
    WebGLProfile {
      vendor: "X.Org",
      renderer: "AMD Radeon RX 580 (polaris10, LLVM 15.0.7, DRM 3.49, 6.1.0)",
    },
    WebGLProfile {
      vendor: "Mesa",
      renderer: "llvmpipe (LLVM 15.0.7, 256 bits)",
    },
  ]
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_profiles_for_known_os() {
    assert!(!get_webgl_profiles_for_os("windows").is_empty());
    assert!(!get_webgl_profiles_for_os("macos").is_empty());
    assert!(!get_webgl_profiles_for_os("linux").is_empty());
  }

  #[test]
  fn test_default_falls_back_to_windows() {
    let default = get_webgl_profiles_for_os("unknown");
    let windows = get_webgl_profiles_for_os("windows");
    assert_eq!(default.len(), windows.len());
  }

  #[test]
  fn test_random_profile_deterministic() {
    let a = get_random_webgl_profile("windows", 42);
    let b = get_random_webgl_profile("windows", 42);
    assert_eq!(a.vendor, b.vendor);
    assert_eq!(a.renderer, b.renderer);
  }

  #[test]
  fn test_random_profile_wraps_around() {
    let profiles = get_webgl_profiles_for_os("macos");
    let len = profiles.len() as u32;
    let a = get_random_webgl_profile("macos", 0);
    let b = get_random_webgl_profile("macos", len);
    assert_eq!(a.vendor, b.vendor);
    assert_eq!(a.renderer, b.renderer);
  }

  #[test]
  fn test_macos_vendors_are_apple() {
    for profile in get_webgl_profiles_for_os("macos") {
      assert_eq!(profile.vendor, "Apple");
    }
  }

  #[test]
  fn test_windows_vendors_are_google_angle() {
    for profile in get_webgl_profiles_for_os("windows") {
      assert!(
        profile.vendor.starts_with("Google Inc."),
        "Expected Google Inc. vendor, got: {}",
        profile.vendor
      );
    }
  }
}
