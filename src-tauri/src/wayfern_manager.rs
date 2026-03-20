use crate::browser_runner::BrowserRunner;
use crate::profile::BrowserProfile;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::process::Command as TokioCommand;
use tokio::sync::Mutex as AsyncMutex;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub type WayfernConfig = ChromiumConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChromiumConfig {
  #[serde(default)]
  pub seed: Option<u32>, // --fingerprint=<seed>
  #[serde(default)]
  pub os: Option<String>, // --fingerprint-platform= (windows|linux|macos)
  #[serde(default)]
  pub brand: Option<String>, // --fingerprint-brand= (Chrome|Edge)
  #[serde(default)]
  pub hardware_concurrency: Option<u32>, // --fingerprint-hardware-concurrency=
  #[serde(default)]
  pub timezone: Option<String>, // --timezone=
  #[serde(default)]
  pub lang: Option<String>, // --lang=
  #[serde(default)]
  pub randomize_fingerprint_on_launch: Option<bool>,
  #[serde(default)]
  pub geoip: Option<serde_json::Value>,
  #[serde(default)]
  pub block_webrtc: Option<bool>,
  #[serde(default)]
  pub executable_path: Option<String>,
  #[serde(default, skip_serializing)]
  pub proxy: Option<String>,
  // Keep for backward compat with old profiles - ignored at runtime
  #[serde(default)]
  pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct WayfernLaunchResult {
  pub id: String,
  #[serde(alias = "process_id")]
  pub processId: Option<u32>,
  #[serde(alias = "profile_path")]
  pub profilePath: Option<String>,
  pub url: Option<String>,
  pub cdp_port: Option<u16>,
}

#[derive(Debug)]
struct WayfernInstance {
  #[allow(dead_code)]
  id: String,
  process_id: Option<u32>,
  profile_path: Option<String>,
  url: Option<String>,
  cdp_port: Option<u16>,
}

struct WayfernManagerInner {
  instances: HashMap<String, WayfernInstance>,
}

pub type WayfernManager = ChromiumManager;

pub struct ChromiumManager {
  inner: Arc<AsyncMutex<WayfernManagerInner>>,
  http_client: Client,
}

#[derive(Debug, Deserialize)]
struct CdpTarget {
  #[serde(rename = "type")]
  target_type: String,
  #[serde(rename = "webSocketDebuggerUrl")]
  websocket_debugger_url: Option<String>,
}

impl ChromiumManager {
  fn new() -> Self {
    Self {
      inner: Arc::new(AsyncMutex::new(WayfernManagerInner {
        instances: HashMap::new(),
      })),
      http_client: Client::new(),
    }
  }

  pub fn instance() -> &'static WayfernManager {
    &WAYFERN_MANAGER
  }

  #[allow(dead_code)]
  pub fn get_profiles_dir(&self) -> PathBuf {
    crate::app_dirs::profiles_dir()
  }

  #[allow(dead_code)]
  fn get_binaries_dir(&self) -> PathBuf {
    crate::app_dirs::binaries_dir()
  }

  /// On Linux, when the fingerprint targets a different OS (macOS/Windows),
  /// generate a fontconfig configuration that includes font files matching
  /// the target platform. This uses fonts bundled with Camoufox.
  /// Returns the path to the fontconfig directory if successful.
  #[cfg(target_os = "linux")]
  fn setup_cross_os_fontconfig(target_os: &str) -> Option<String> {
    let (font_subdir, fontconfig_subdir) = match target_os {
      "macos" => ("macos", "fontconfig-macos"),
      "windows" => ("windows", "fontconfig-windows"),
      _ => return None,
    };

    let data_dir = crate::app_dirs::data_dir();
    let fontconfig_dir = data_dir.join(fontconfig_subdir);

    // Find Camoufox font directory: binaries/camoufox/<version>/fonts/<os>/
    let camoufox_base = data_dir.join("binaries").join("camoufox");
    let font_dirs = Self::find_camoufox_font_dirs(&camoufox_base, font_subdir);

    if font_dirs.is_empty() {
      log::warn!(
        "No Camoufox {font_subdir} fonts found under {}, cross-OS fontconfig not available",
        camoufox_base.display()
      );
      return None;
    }

    // Generate fontconfig
    if let Err(e) = Self::write_fontconfig(&fontconfig_dir, target_os, &font_dirs) {
      log::warn!("Failed to write fontconfig for {target_os}: {e}");
      return None;
    }

    Some(fontconfig_dir.to_string_lossy().to_string())
  }

  /// Find Camoufox font directories for a given OS.
  /// Looks in binaries/camoufox/<version>/fonts/<os>/ and its Supplemental subdirectory.
  #[cfg(target_os = "linux")]
  fn find_camoufox_font_dirs(camoufox_base: &std::path::Path, font_subdir: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    let Ok(entries) = std::fs::read_dir(camoufox_base) else {
      return dirs;
    };

    // Find the most recent version directory
    let mut versions: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    versions.sort();

    for version_dir in versions.iter().rev() {
      let fonts_dir = version_dir.join("fonts").join(font_subdir);
      if fonts_dir.is_dir() {
        dirs.push(fonts_dir.clone());
        let supplemental = fonts_dir.join("Supplemental");
        if supplemental.is_dir() {
          dirs.push(supplemental);
        }
        break;
      }
    }

    dirs
  }

  /// Write a fontconfig fonts.conf file for the given target OS.
  #[cfg(target_os = "linux")]
  fn write_fontconfig(
    fontconfig_dir: &std::path::Path,
    target_os: &str,
    font_dirs: &[PathBuf],
  ) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(fontconfig_dir)?;

    let font_dir_entries: String = font_dirs
      .iter()
      .map(|d| format!("\t<dir>{}</dir>", d.display()))
      .collect::<Vec<_>>()
      .join("\n");

    let os_aliases = match target_os {
      "macos" => include_str!("fontconfig_macos.xml"),
      "windows" => include_str!("fontconfig_windows.xml"),
      _ => "",
    };

    let content = format!(
      r#"<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "fonts.dtd">
<fontconfig>

<!-- Target platform ({target_os}) font directories -->
{font_dir_entries}

<!-- System font directories as fallback -->
	<dir>/usr/share/fonts</dir>
	<dir>/usr/local/share/fonts</dir>

{os_aliases}

<!-- Font cache directory list -->
	<cachedir prefix="xdg">fontconfig</cachedir>

	<config>
		<rescan>
			<int>30</int>
		</rescan>
	</config>

	<!-- Standardize rendering settings -->
	<match target="pattern">
		<edit name="antialias" mode="assign"><bool>true</bool></edit>
		<edit name="autohint" mode="assign"><bool>false</bool></edit>
		<edit name="hinting" mode="assign"><bool>true</bool></edit>
		<edit name="hintstyle" mode="assign"><const>hintfull</const></edit>
		<edit name="lcdfilter" mode="assign"><const>lcddefault</const></edit>
		<edit name="rgba" mode="assign"><const>none</const></edit>
	</match>
</fontconfig>
"#
    );

    let conf_path = fontconfig_dir.join("fonts.conf");
    std::fs::write(&conf_path, content)?;
    log::info!(
      "Wrote fontconfig for {target_os} at {}",
      conf_path.display()
    );
    Ok(())
  }

  async fn find_free_port() -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
  }

  async fn wait_for_cdp_ready(
    &self,
    port: u16,
  ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("http://127.0.0.1:{port}/json/version");
    let max_attempts = 50;
    let delay = Duration::from_millis(100);

    for attempt in 0..max_attempts {
      match self.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
          log::info!("CDP ready on port {port} after {attempt} attempts");
          return Ok(());
        }
        _ => {
          tokio::time::sleep(delay).await;
        }
      }
    }

    Err(format!("CDP not ready after {max_attempts} attempts on port {port}").into())
  }

  async fn get_cdp_targets(
    &self,
    port: u16,
  ) -> Result<Vec<CdpTarget>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("http://127.0.0.1:{port}/json");
    let resp = self.http_client.get(&url).send().await?;
    let targets: Vec<CdpTarget> = resp.json().await?;
    Ok(targets)
  }

  async fn send_cdp_command(
    &self,
    ws_url: &str,
    method: &str,
    params: serde_json::Value,
  ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let (mut ws_stream, _) = connect_async(ws_url).await?;

    let command = json!({
      "id": 1,
      "method": method,
      "params": params
    });

    use futures_util::sink::SinkExt;
    use futures_util::stream::StreamExt;

    ws_stream
      .send(Message::Text(command.to_string().into()))
      .await?;

    while let Some(msg) = ws_stream.next().await {
      match msg? {
        Message::Text(text) => {
          let response: serde_json::Value = serde_json::from_str(text.as_str())?;
          if response.get("id") == Some(&json!(1)) {
            if let Some(error) = response.get("error") {
              return Err(format!("CDP error: {}", error).into());
            }
            return Ok(response.get("result").cloned().unwrap_or(json!({})));
          }
        }
        Message::Close(_) => break,
        _ => {}
      }
    }

    Err("No response received from CDP".into())
  }

  pub async fn generate_fingerprint_config(
    &self,
    _app_handle: &AppHandle,
    _profile: &BrowserProfile,
    config: &WayfernConfig,
  ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // fingerprint-chromium uses deterministic seeds: just generate a random one
    let seed = config.seed.unwrap_or_else(rand::random::<u32>);
    log::info!("Generated fingerprint-chromium seed: {seed}");
    Ok(seed.to_string())
  }

  #[allow(clippy::too_many_arguments)]
  pub async fn launch_chromium(
    &self,
    _app_handle: &AppHandle,
    profile: &BrowserProfile,
    profile_path: &str,
    config: &WayfernConfig,
    url: Option<&str>,
    proxy_url: Option<&str>,
    ephemeral: bool,
    extension_paths: &[String],
    remote_debugging_port: Option<u16>,
  ) -> Result<WayfernLaunchResult, Box<dyn std::error::Error + Send + Sync>> {
    let executable_path = if let Some(path) = &config.executable_path {
      let p = PathBuf::from(path);
      if p.exists() {
        p
      } else {
        log::warn!("Stored Wayfern executable path does not exist: {path}, falling back to dynamic resolution");
        BrowserRunner::instance()
          .get_browser_executable_path(profile)
          .map_err(|e| format!("Failed to get Wayfern executable path: {e}"))?
      }
    } else {
      BrowserRunner::instance()
        .get_browser_executable_path(profile)
        .map_err(|e| format!("Failed to get Wayfern executable path: {e}"))?
    };

    let port = match remote_debugging_port {
      Some(p) => p,
      None => Self::find_free_port().await?,
    };
    log::info!("Launching fingerprint-chromium on CDP port {port}");

    let mut args = vec![
      format!("--remote-debugging-port={port}"),
      "--remote-debugging-address=127.0.0.1".to_string(),
      format!("--user-data-dir={}", profile_path),
      "--no-first-run".to_string(),
      "--no-default-browser-check".to_string(),
      "--disable-background-mode".to_string(),
      "--disable-component-update".to_string(),
      "--disable-background-timer-throttling".to_string(),
      "--crash-server-url=".to_string(),
      "--disable-updater".to_string(),
      "--disable-session-crashed-bubble".to_string(),
      "--hide-crash-restore-bubble".to_string(),
      "--disable-infobars".to_string(),
      "--disable-quic".to_string(),
      "--disable-features=DialMediaRouteProvider".to_string(),
      "--use-mock-keychain".to_string(),
      "--password-store=basic".to_string(),
      // Use ANGLE with native GL backend (Mesa llvmpipe) instead of SwiftShader.
      // This avoids the failIfMajorPerformanceCaveat detection that pixelscan uses
      // to detect software rendering. Mesa's llvmpipe is treated as a real GL driver
      // by ANGLE, so WebGL contexts with failIfMajorPerformanceCaveat:true succeed.
      "--use-gl=angle".to_string(),
      "--use-angle=gl".to_string(),
      "--ignore-gpu-blocklist".to_string(),
      // Disable fingerprint-chromium's seed-based canvas noise.
      // Pixelscan detects injected noise via double-render comparison.
      "--disable-spoofing=canvas".to_string(),
    ];

    // Build fingerprint CLI args for fingerprint-chromium
    // The "fingerprint" field now stores the seed (a u32 string) from generate_fingerprint_config
    let seed = config
      .fingerprint
      .as_deref()
      .and_then(|s| s.parse::<u32>().ok())
      .or(config.seed)
      .unwrap_or_else(rand::random::<u32>);
    args.push(format!("--fingerprint={seed}"));
    log::info!(
      "Using fingerprint seed: {seed}, config.os: {:?}, config.brand: {:?}",
      config.os,
      config.brand
    );

    if let Some(ref os) = config.os {
      args.push(format!("--fingerprint-platform={os}"));
      log::info!("Set fingerprint platform: {os}");
    } else {
      log::warn!("No OS set in wayfern config — fingerprint will use native OS");
    }
    if let Some(ref brand) = config.brand {
      args.push(format!("--fingerprint-brand={brand}"));
    }
    if let Some(hw) = config.hardware_concurrency {
      args.push(format!("--fingerprint-hardware-concurrency={hw}"));
    }
    if let Some(ref tz) = config.timezone {
      args.push(format!("--timezone={tz}"));
    }
    if let Some(ref lang) = config.lang {
      args.push(format!("--lang={lang}"));
    }

    if let Some(proxy) = proxy_url {
      args.push(format!("--proxy-server={proxy}"));
    }

    // Resolve geolocation and pass via CLI arg (--fingerprint-location) instead of CDP.
    // Using CDP Emulation.setGeolocationOverride on a page target leaves detectable traces
    // that cause jsModifyDetected=true on fingerprint scanners. fingerprint-chromium supports
    // native C++ geolocation spoofing via the --fingerprint-location CLI arg.
    let geoip_option = config.geoip.as_ref();
    let should_geolocate = !matches!(geoip_option, Some(serde_json::Value::Bool(false)));

    if should_geolocate {
      let geo_result = async {
        let ip = match geoip_option {
          Some(serde_json::Value::String(ip_str)) => ip_str.clone(),
          _ => crate::ip_utils::fetch_public_ip(config.proxy.as_deref())
            .await
            .map_err(|e| format!("Failed to fetch public IP: {e}"))?,
        };
        crate::camoufox::geolocation::get_geolocation(&ip)
          .map_err(|e| format!("Failed to get geolocation for IP {ip}: {e}"))
      }
      .await;

      match geo_result {
        Ok(geo) => {
          let lat = geo.latitude;
          let lng = geo.longitude;
          args.push(format!("--fingerprint-location={lat},{lng}"));
          log::info!("Set fingerprint geolocation via CLI: lat={lat}, lng={lng}");
        }
        Err(e) => {
          log::warn!("Geolocation lookup failed, skipping location spoofing: {e}");
        }
      }
    }

    if ephemeral {
      args.push("--disk-cache-size=1".to_string());
      args.push("--disable-breakpad".to_string());
      args.push("--disable-crash-reporter".to_string());
      args.push("--no-service-autorun".to_string());
      args.push("--disable-sync".to_string());
    }

    if !extension_paths.is_empty() {
      args.push(format!("--load-extension={}", extension_paths.join(",")));
    }

    // Pass URL as last CLI arg (standard Chromium behavior)
    if let Some(url) = url {
      args.push(url.to_string());
    }

    let mut cmd = TokioCommand::new(&executable_path);
    cmd.args(&args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Set Mesa env vars to make ANGLE's native GL backend pass
    // failIfMajorPerformanceCaveat checks even on software rendering
    #[cfg(target_os = "linux")]
    {
      cmd.env("MESA_GL_VERSION_OVERRIDE", "4.5");
      cmd.env("MESA_GLSL_VERSION_OVERRIDE", "450");
    }

    // On Linux, when fingerprinting a different OS, set up fontconfig to use
    // fonts that match the target platform (bundled with Camoufox).
    #[cfg(target_os = "linux")]
    if let Some(ref os) = config.os {
      if let Some(fontconfig_path) = Self::setup_cross_os_fontconfig(os) {
        cmd.env("FONTCONFIG_PATH", &fontconfig_path);
        log::info!("Set FONTCONFIG_PATH={fontconfig_path} for cross-OS font rendering");
      }
    }

    let child = cmd.spawn()?;
    let process_id = child.id();

    self.wait_for_cdp_ready(port).await?;

    let id = uuid::Uuid::new_v4().to_string();
    let instance = WayfernInstance {
      id: id.clone(),
      process_id,
      profile_path: Some(profile_path.to_string()),
      url: url.map(|s| s.to_string()),
      cdp_port: Some(port),
    };

    let mut inner = self.inner.lock().await;
    inner.instances.insert(id.clone(), instance);

    Ok(WayfernLaunchResult {
      id,
      processId: process_id,
      profilePath: Some(profile_path.to_string()),
      url: url.map(|s| s.to_string()),
      cdp_port: Some(port),
    })
  }

  pub async fn stop_chromium(
    &self,
    id: &str,
  ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut inner = self.inner.lock().await;

    if let Some(instance) = inner.instances.remove(id) {
      if let Some(pid) = instance.process_id {
        #[cfg(unix)]
        {
          use nix::sys::signal::{kill, Signal};
          use nix::unistd::Pid;
          let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
        }
        #[cfg(windows)]
        {
          use std::os::windows::process::CommandExt;
          const CREATE_NO_WINDOW: u32 = 0x08000000;
          let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        }
        log::info!("Stopped Wayfern instance {id} (PID: {pid})");
      }
    }

    Ok(())
  }

  /// Opens a URL in a new tab for an existing Wayfern instance using CDP.
  /// Returns Ok(()) if successful, or an error if the instance is not found or CDP fails.
  pub async fn open_url_in_tab(
    &self,
    profile_path: &str,
    url: &str,
  ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let instance = self
      .find_chromium_by_profile(profile_path)
      .await
      .ok_or("Wayfern instance not found for profile")?;

    let cdp_port = instance
      .cdp_port
      .ok_or("No CDP port available for Wayfern instance")?;

    // Get the browser target to create a new tab
    let targets = self.get_cdp_targets(cdp_port).await?;

    // Find a page target to get the WebSocket URL (we need any target to send commands)
    let page_target = targets
      .iter()
      .find(|t| t.target_type == "page" && t.websocket_debugger_url.is_some())
      .ok_or("No page target found for CDP")?;

    let ws_url = page_target
      .websocket_debugger_url
      .as_ref()
      .ok_or("No WebSocket URL available")?;

    // Use Target.createTarget to open a new tab with the URL
    self
      .send_cdp_command(ws_url, "Target.createTarget", json!({ "url": url }))
      .await?;

    log::info!("Opened URL in new tab via CDP: {}", url);
    Ok(())
  }

  pub async fn get_cdp_port(&self, profile_path: &str) -> Option<u16> {
    let inner = self.inner.lock().await;
    let target_path = std::path::Path::new(profile_path)
      .canonicalize()
      .unwrap_or_else(|_| std::path::Path::new(profile_path).to_path_buf());

    for instance in inner.instances.values() {
      if let Some(path) = &instance.profile_path {
        let instance_path = std::path::Path::new(path)
          .canonicalize()
          .unwrap_or_else(|_| std::path::Path::new(path).to_path_buf());
        if instance_path == target_path {
          return instance.cdp_port;
        }
      }
    }
    None
  }

  pub async fn find_chromium_by_profile(&self, profile_path: &str) -> Option<WayfernLaunchResult> {
    use sysinfo::{ProcessRefreshKind, RefreshKind, System};

    let mut inner = self.inner.lock().await;

    // Canonicalize the target path for comparison
    let target_path = std::path::Path::new(profile_path)
      .canonicalize()
      .unwrap_or_else(|_| std::path::Path::new(profile_path).to_path_buf());

    // Find the instance with the matching profile path
    let mut found_id: Option<String> = None;
    for (id, instance) in &inner.instances {
      if let Some(path) = &instance.profile_path {
        let instance_path = std::path::Path::new(path)
          .canonicalize()
          .unwrap_or_else(|_| std::path::Path::new(path).to_path_buf());
        if instance_path == target_path {
          found_id = Some(id.clone());
          break;
        }
      }
    }

    // If we found an instance, verify the process is still running
    if let Some(id) = found_id {
      if let Some(instance) = inner.instances.get(&id) {
        if let Some(pid) = instance.process_id {
          let system = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
          );
          let sysinfo_pid = sysinfo::Pid::from_u32(pid);

          if system.process(sysinfo_pid).is_some() {
            return Some(WayfernLaunchResult {
              id: id.clone(),
              processId: instance.process_id,
              profilePath: instance.profile_path.clone(),
              url: instance.url.clone(),
              cdp_port: instance.cdp_port,
            });
          } else {
            log::info!(
              "Wayfern process {} for profile {} is no longer running, cleaning up",
              pid,
              profile_path
            );
            inner.instances.remove(&id);
            return None;
          }
        }
      }
    }

    // If not found in in-memory instances, scan system processes.
    // This handles the case where the GUI was restarted but Wayfern is still running.
    if let Some((pid, found_profile_path, cdp_port)) =
      Self::find_chromium_process_by_profile(&target_path)
    {
      log::info!(
        "Found running Wayfern process (PID: {}) for profile path via system scan",
        pid
      );

      let instance_id = format!("recovered_{}", pid);
      inner.instances.insert(
        instance_id.clone(),
        WayfernInstance {
          id: instance_id.clone(),
          process_id: Some(pid),
          profile_path: Some(found_profile_path.clone()),
          url: None,
          cdp_port,
        },
      );

      return Some(WayfernLaunchResult {
        id: instance_id,
        processId: Some(pid),
        profilePath: Some(found_profile_path),
        url: None,
        cdp_port,
      });
    }

    None
  }

  /// Scan system processes to find a Wayfern/Chromium process using a specific profile path
  fn find_chromium_process_by_profile(
    target_path: &std::path::Path,
  ) -> Option<(u32, String, Option<u16>)> {
    use sysinfo::{ProcessRefreshKind, RefreshKind, System};

    let system = System::new_with_specifics(
      RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );

    let target_path_str = target_path.to_string_lossy();

    for (pid, process) in system.processes() {
      let cmd = process.cmd();
      if cmd.is_empty() {
        continue;
      }

      let exe_name = process.name().to_string_lossy().to_lowercase();
      let is_chromium_like = exe_name.contains("wayfern")
        || exe_name.contains("chromium")
        || exe_name.contains("chrome");

      if !is_chromium_like {
        continue;
      }

      // Skip child processes (renderer, GPU, utility, zygote, etc.)
      // Only the main browser process lacks a --type= argument
      let is_child = cmd
        .iter()
        .any(|a| a.to_str().is_some_and(|s| s.starts_with("--type=")));
      if is_child {
        continue;
      }

      let mut matched = false;
      let mut cdp_port: Option<u16> = None;

      for arg in cmd.iter() {
        if let Some(arg_str) = arg.to_str() {
          if let Some(dir_val) = arg_str.strip_prefix("--user-data-dir=") {
            let cmd_path = std::path::Path::new(dir_val)
              .canonicalize()
              .unwrap_or_else(|_| std::path::Path::new(dir_val).to_path_buf());
            if cmd_path == target_path {
              matched = true;
            }
          }

          if let Some(port_val) = arg_str.strip_prefix("--remote-debugging-port=") {
            cdp_port = port_val.parse().ok();
          }
        }
      }

      if matched {
        return Some((pid.as_u32(), target_path_str.to_string(), cdp_port));
      }
    }

    None
  }

  #[allow(dead_code)]
  pub async fn launch_chromium_profile(
    &self,
    app_handle: &AppHandle,
    profile: &BrowserProfile,
    config: &WayfernConfig,
    url: Option<&str>,
    proxy_url: Option<&str>,
  ) -> Result<WayfernLaunchResult, Box<dyn std::error::Error + Send + Sync>> {
    let profiles_dir = self.get_profiles_dir();
    let profile_path = profiles_dir.join(profile.id.to_string()).join("profile");
    let profile_path_str = profile_path.to_string_lossy().to_string();

    std::fs::create_dir_all(&profile_path)?;

    if let Some(existing) = self.find_chromium_by_profile(&profile_path_str).await {
      log::info!("Stopping existing Wayfern instance for profile");
      self.stop_chromium(&existing.id).await?;
    }

    self
      .launch_chromium(
        app_handle,
        profile,
        &profile_path_str,
        config,
        url,
        proxy_url,
        profile.ephemeral,
        &[],
        None,
      )
      .await
  }

  #[allow(dead_code)]
  pub async fn cleanup_dead_instances(&self) {
    use sysinfo::{ProcessRefreshKind, RefreshKind, System};

    let mut inner = self.inner.lock().await;
    let mut dead_ids = Vec::new();

    let system = System::new_with_specifics(
      RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );

    for (id, instance) in &inner.instances {
      if let Some(pid) = instance.process_id {
        let pid = sysinfo::Pid::from_u32(pid);
        if !system.processes().contains_key(&pid) {
          dead_ids.push(id.clone());
        }
      }
    }

    for id in dead_ids {
      log::info!("Cleaning up dead Wayfern instance: {id}");
      inner.instances.remove(&id);
    }
  }
}

lazy_static::lazy_static! {
  static ref WAYFERN_MANAGER: ChromiumManager = ChromiumManager::new();
}
