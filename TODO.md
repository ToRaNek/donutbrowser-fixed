# TODO — Verify on Windows/macOS (real hardware)

All testing was done on ChromeOS VM which flags EVERYTHING as "masking detected" (including vanilla Chrome with zero modifications). The fixes below need verification on real Windows/macOS hardware with a GPU.

## Fixes implemented (need verification on real hardware)

### Camoufox (Firefox) — Primary engine for anti-detect
- [x] Cross-OS fingerprint generation (macOS/Windows from Linux)
- [x] Font isolation: hide non-target OS fonts by renaming `fonts/windows` and `fonts/linux` dirs at launch
- [x] WebGL: `webgl.sanitize-unmasked-renderer=false` to prevent "llvmpipe, or similar" leak
- [x] WebGL: `webgl.force-enabled=true` + `webgl.disable-fail-if-major-performance-caveat=true`
- [x] Canvas: `canvas:aaOffset` C++ anti-aliasing modification (no JS injection)
- [x] Audio: seed-based audio fingerprint
- [x] All spoofing at C++ level — no JS injection, no prototype modification
- [x] Bundled fonts for macOS (150+ fonts), Windows (140+ fonts), Linux (140+ fonts)

### fingerprint-chromium (Chromium) — Secondary engine
- [x] Mesa GL backend (`--use-angle=gl`) instead of SwiftShader to bypass `failIfMajorPerformanceCaveat`
- [x] `--disable-spoofing=canvas` for clean canvas rendering
- [x] `MESA_GL_VERSION_OVERRIDE=4.5` env var on Linux
- [x] `--ignore-gpu-blocklist` flag
- [x] Seed-based fingerprinting via `--fingerprint=<seed>`
- [x] Cross-OS via `--fingerprint-platform=macos|windows|linux`
- [ ] **Known issue**: `navigator.userAgentData` (Client Hints) removed by ungoogled-chromium fork — detected by pixelscan as inconsistent. No fix possible without recompiling.

### Proxy fixes
- [x] `Proxy::http()` → `Proxy::all()` for HTTPS routing
- [x] TCP keepalive (30s) on tunnel connections
- [x] SOCKS4 DNS resolution
- [x] Request timeouts (10s connect, 120s request)
- [x] Connection pooling

### UI/Features
- [x] Cross-OS fingerprints unlocked (Windows/macOS/Linux)
- [x] All pro features enabled
- [x] Toast notifications moved to top-right
- [x] Wayfern replaced with fingerprint-chromium (open-source)

## To verify on Windows with GPU

1. Create Camoufox profile with macOS fingerprint → run pixelscan
2. Create Camoufox profile with Windows fingerprint (native) → run pixelscan
3. Create Chromium profile with macOS fingerprint → run pixelscan
4. Check that WebGL shows real GPU (not llvmpipe/SwiftShader)
5. Check that fonts match target OS
6. Check `noNoise`, `jsModifyDetected`, `webglStatus`, `fonts` all pass

## Known limitations

- **ChromeOS/VM**: Pixelscan detects ChromeOS/VM environments regardless of anti-detect. All browsers (including vanilla Chrome) show "masking detected" on ChromeOS.
- **fingerprint-chromium Client Hints**: `navigator.userAgentData` is null (ungoogled-chromium removes it). This is detectable.
- **Camoufox font isolation**: Requires renaming font directories at launch time. Implementation in `camoufox_manager.rs` needed.
- **Camoufox DPR**: `devicePixelRatio` config is ignored, uses display DPR. Resolution shows doubled values on HiDPI displays.
