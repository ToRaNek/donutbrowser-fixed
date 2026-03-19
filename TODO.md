# Improvements TODO

## Cross-OS Fingerprint Detection Issues

### Chromium (fingerprint-chromium) on Windows
- **Bug**: Setting macOS fingerprint on Windows still shows "Windows" on pixelscan
- **Cause**: `--fingerprint-platform=macos` flag may not be passed correctly, or the Windows build of fingerprint-chromium doesn't fully support cross-OS platform spoofing
- **Fix needed**: Debug the actual CLI args passed to the binary on Windows and verify fingerprint-chromium Windows builds support `--fingerprint-platform`

### Fonts mismatch (`fonts: false` on pixelscan)
- **Cause**: The host OS has Linux/Windows fonts installed, but the fingerprint claims macOS (or vice versa). Pixelscan probes for OS-specific fonts (e.g. San Francisco on macOS, Segoe UI on Windows) and detects the mismatch.
- **Fix**: Bundle OS-specific font lists per platform and load them via Camoufox's custom font system or fingerprint-chromium's font spoofing. Camoufox already has a `fonts.json` data file and custom font loading support — extend it with per-OS font sets.

### WebGL empty (`webglStatus: ["-", "-", "-"]`)
- **Cause**: WebGL data (vendor, renderer, extensions) is not populated in cross-OS profiles. The GPU on the host doesn't match the spoofed OS, and no fallback WebGL data is injected.
- **Fix**: Use the `webgl_data.db` database already embedded in Camoufox to inject realistic WebGL parameters matching the target OS. For fingerprint-chromium, investigate if `--fingerprint=<seed>` generates WebGL data or if additional flags are needed.

### Canvas noise detected (`noNoise: false`)
- **Cause**: Canvas fingerprint noise is applied but pixelscan detects the noise pattern as artificial (statistical analysis of pixel variations).
- **Fix**: This is an arms race issue. Camoufox uses C++-level canvas injection which is harder to detect than JS-level. For fingerprint-chromium, the noise is seed-based and deterministic. Improving this requires changes in the upstream browser forks.

### JS modification detected (`jsModifyDetected: true`)
- **Cause**: Camoufox (Firefox) modifies JS APIs at the C++ level, but some detection scripts still identify inconsistencies in the prototype chain or timing differences.
- **Fix**: Upstream Camoufox issue. Monitor Camoufox releases for improvements. For fingerprint-chromium, this should not appear since spoofing is at the Chromium source level.

## Feature Improvements

### Rename "Wayfern" to "Chromium" in UI
- The internal identifier `"wayfern"` is still used everywhere in the codebase and displayed in the UI
- All i18n locale files reference "Wayfern" (en.json, fr.json, es.json, etc.)
- Should be renamed to "Chromium" or "Fingerprint Chromium" for clarity
- Note: changing the internal `"wayfern"` string requires profile migration (existing profiles store `browser: "wayfern"`)

### Auto-download and extraction of fingerprint-chromium
- The download from GitHub releases works but extraction sometimes fails silently
- The tar.xz archive extracts into a subdirectory (`ungoogled-chromium-X.X.X-1-x86_64_linux/`) that needs to be flattened
- Need to add post-extraction logic to move files from the subdirectory to the version root
- Also need to handle different archive formats per platform (tar.xz on Linux, zip on Windows, dmg on macOS)

### Client Hints consistency
- On Windows with Chromium, `identicalCH: false` was reported — Client Hints headers don't match the User-Agent
- fingerprint-chromium should handle this via the seed, but cross-OS may break CH consistency
- Investigate if `--fingerprint-brand` and `--fingerprint-brand-version` need to be explicitly set

### Geolocation spoofing improvements
- Current implementation uses IP-based geolocation via GeoIP database
- Timezone is set via `--timezone` flag for fingerprint-chromium and via config for Camoufox
- Could improve by also matching locale/language to the proxy's geographic location automatically

### Per-OS font databases
- Build font list databases for Windows, macOS, and Linux
- When creating a cross-OS profile, inject the correct font list for the target OS
- Camoufox already supports custom font loading — leverage this

### WebGL spoofing per OS
- Build a database of realistic GPU vendor/renderer strings per OS
- Windows: "ANGLE (NVIDIA GeForce RTX 3060)", "ANGLE (Intel UHD 630)", etc.
- macOS: "Apple M1", "Apple M2 Pro", "Intel Iris Plus", etc.
- Linux: "Mesa Intel UHD 630", "NVIDIA GeForce GTX 1080/PCIe/SSE2", etc.
- Inject matching WebGL data when creating cross-OS profiles

## Architecture Improvements

### Decouple fingerprint generation from browser engine
- Currently Camoufox (Firefox) has its own Bayesian network generator, and fingerprint-chromium uses seed-based generation
- Could unify into a single fingerprint generation system that produces consistent data for both engines
- The Camoufox Bayesian network data could be used to generate richer fingerprints for fingerprint-chromium profiles too

### Profile migration system
- No formal migration system exists for profile config changes
- Old profiles with Wayfern JSON blob fingerprints should be automatically migrated to seed-based config
- Could add a version field to profile metadata and run migrations on load
