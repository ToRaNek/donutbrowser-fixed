# Improvements TODO

## Pixelscan Detection Issues

### Current Status
On Linux with cross-OS profiles (macOS/Windows), pixelscan reports "Masking detected" with:
- `fonts: false` — Font set doesn't match claimed OS
- `noNoise: false` — Canvas noise pattern detected
- `webglStatus: ["-","-","-"]` — WebGL data empty
- `jsModifyDetected: true` — JS API modifications detected

The correct OS is shown (e.g., "Chrome 144 on Mac OS") — cross-OS spoofing works, but the fingerprint details are flagged.

### Chromium (fingerprint-chromium) on Windows
- **Bug**: Setting macOS fingerprint on Windows still shows "Windows" on pixelscan
- `--fingerprint-platform=macos` may not work on Windows builds
- Need to debug actual CLI args on Windows platform

---

## Root Cause Analysis

### fonts: false
**Cause:** Host OS has Linux/Windows fonts, but fingerprint claims macOS (or vice versa). Pixelscan probes for OS-specific fonts using `measureText()` and CSS fallback measurement, then checks if fonts match claimed OS.

**How commercial browsers solve it:**
- **Camoufox** bundles real font files for all 3 OSes + uses fontconfig whitelisting to hide host OS fonts
- **Octo Browser** uses "fingerprints from real devices" with curated font sets
- **Multilogin** is the only one doing true cross-OS font emulation

**Fix options:**
1. Bundle per-OS font files (macOS: San Francisco, Helvetica Neue; Windows: Segoe UI, Calibri) and load them via fontconfig whitelisting
2. For fingerprint-chromium: font metrics are perturbed by the seed at C++ level, but actual font rendering still uses host fonts. Need to install target OS fonts on host system for true cross-OS
3. Camoufox already has this infrastructure (`fonts.json`, `FONTCONFIG_PATH`) — verify it's working

### noNoise: false
**Cause:** Canvas noise is applied but pixelscan detects it as artificial via:
- Double-render comparison (same canvas drawn twice, hashes differ = noise)
- Known-pixel verification (fill exact RGBA, check for deviation)
- ML database comparison (hash matches no known real device)

**How commercial browsers solve it:**
- **Camoufox** uses closed-source Skia C++ patch that modifies anti-aliasing algorithm (not pixel values). Uses `canvas:aaOffset` (-50 to 50) for deterministic, session-consistent output
- **fingerprint-chromium** applies seed-deterministic pixel noise at C++ level (improved in Chrome 142)
- **Kameleo** maps canvas output to match real device configurations

**Current code:**
- Camoufox: `canvas:aaOffset` correctly set in `config.rs:399-403`
- fingerprint-chromium: relies on `--fingerprint=<seed>` for C++ level noise

**Fix:** This is largely an upstream issue. Ensure using fingerprint-chromium >= 142. The double-render test should pass (deterministic), but ML comparison may fail. No easy fix without upstream changes.

### webglStatus: ["-","-","-"]
**Cause:** WebGL vendor/renderer/extensions not populated in cross-OS profiles.

**How it works:**
- Pixelscan checks: unmasked vendor (`gl.getParameter(0x9245)`), unmasked renderer (`gl.getParameter(0x9246)`), WebGL render hash
- Cross-validates against UA OS, GPU capabilities, extensions, shader precision

**How commercial browsers solve it:**
- **Camoufox** samples from `webgl_data.db` (SQLite) with OS-weighted probabilities
- **fingerprint-chromium** auto-generates WebGL from seed since v139, fully in v144

**Fix:** fingerprint-chromium v144 should handle this natively via seed. If empty, check that `--disable-spoofing=gpu` is NOT being set. The `--fingerprint=<seed>` flag should auto-generate GPU data.

### jsModifyDetected: true
**Cause:** Detection scripts check for JS API modifications via:
- `Function.prototype.toString` analysis
- `Object.getOwnPropertyDescriptor` checks
- Worker scope comparison
- Proxy trap detection
- Error stack trace analysis

**How commercial browsers solve it:**
- All modifications at C++ engine level — JS sees authentic native functions
- `Object.getOwnPropertyDescriptor` returns `undefined` for native properties
- Values match in Worker contexts (C++ affects all contexts)

**For fingerprint-chromium:** All spoofing is C++ level. `jsModifyDetected` should be FALSE. If it appears:
1. Check no browser extensions are injecting JS (the `font-spoof` extension in `src-tauri/extensions/` exists but should NOT be loaded)
2. Check no CDP `Runtime.evaluate` calls are modifying JS APIs
3. The `--disable-non-proxied-udp` flag is fine (standard Chromium)

**For Camoufox:** Same — all C++ level. Should not trigger. If it does, check for extensions modifying JS.

---

## Priority Actions

### P0 — Investigate jsModifyDetected on fingerprint-chromium
fingerprint-chromium does all spoofing at C++ level. `jsModifyDetected: true` should not happen. Steps:
1. Launch a profile with NO extensions, NO proxy
2. Check if jsModifyDetected is still true
3. If yes, it's a fingerprint-chromium upstream issue
4. If no, an extension or CDP call is the cause

### P1 — Cross-OS font matching
The hardest problem. Options ranked by effort:
1. **Low effort:** Only allow profiles matching host OS for Chromium. Use Camoufox for cross-OS (it has font bundling)
2. **Medium effort:** Install target OS fonts on the host. For Linux: install macOS/Windows core fonts via fontconfig
3. **High effort:** Bundle font files per OS in the app, set up fontconfig whitelisting for fingerprint-chromium (similar to Camoufox)

### P2 — WebGL on cross-OS
Verify fingerprint-chromium v144 generates WebGL data from seed. If it doesn't:
- Check if `--disable-gpu` or `--disable-spoofing=gpu` is accidentally set
- Try without `--headless` (WebGL may need GPU access)
- As fallback, for versions < 144, pass `--fingerprint-gpu-vendor` and `--fingerprint-gpu-renderer`

### P3 — Canvas noise
Upstream limitation for both engines. Monitor fingerprint-chromium and Camoufox releases for improvements. No easy local fix.

---

## Feature Improvements

### Rename "Wayfern" to "Chromium" in UI
- Internal identifier still uses `"wayfern"` in some places
- i18n locale files partially updated
- Profile migration needed (existing profiles store `browser: "wayfern"`)

### Auto-download extraction fix
- tar.xz archives extract into subdirectory, needs flattening
- Handle per-platform archive formats (tar.xz Linux, zip Windows, dmg macOS)

### Geolocation improvements
- Auto-match locale/language to proxy geographic location
- Already implemented via GeoIP database, but could be improved
