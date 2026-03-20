<div align="center">
  <img src="assets/logo.png" alt="Donut Browser Logo" width="150">
  <h1>Donut Browser (Fixed)</h1>
  <strong>Anti-detect browser with proxy fixes, cross-OS fingerprints unlocked, and all features enabled.</strong>
</div>
<br>

<p align="center">
  <a href="https://github.com/ToRaNek/donutbrowser-fixed/releases/latest"><img alt="GitHub release" src="https://img.shields.io/github/v/release/ToRaNek/donutbrowser-fixed"></a>
  <a href="https://github.com/ToRaNek/donutbrowser-fixed/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-AGPL--3.0-blue.svg" alt="License"></a>
</p>

## Download

Choose the right file for your system:

### Windows

| File | Description |
|---|---|
| [Donut_x64-setup.exe](https://github.com/ToRaNek/donutbrowser-fixed/releases/latest/download/Donut_0.19.0_x64-setup.exe) | Windows installer (x64) |

### macOS

| File | Description |
|---|---|
| [Donut_aarch64.dmg](https://github.com/ToRaNek/donutbrowser-fixed/releases/latest/download/Donut_0.19.0_aarch64.dmg) | macOS Apple Silicon (M1/M2/M3/M4) |
| [Donut_x64.dmg](https://github.com/ToRaNek/donutbrowser-fixed/releases/latest/download/Donut_0.19.0_x64.dmg) | macOS Intel |

> **Which macOS version?** If your Mac is from 2020 or later, use **aarch64** (Apple Silicon). Older Macs use **x64** (Intel).

### Linux

| File | Description |
|---|---|
| [Donut_amd64.deb](https://github.com/ToRaNek/donutbrowser-fixed/releases/latest/download/Donut_0.19.0_amd64.deb) | Debian / Ubuntu / Mint |
| [Donut_x86_64.rpm](https://github.com/ToRaNek/donutbrowser-fixed/releases/latest/download/Donut-0.19.0-1.x86_64.rpm) | Fedora / RHEL / openSUSE |
| [Donut_amd64.AppImage](https://github.com/ToRaNek/donutbrowser-fixed/releases/latest/download/Donut_0.19.0_amd64.AppImage) | Universal (any distro) |

> **Which Linux version?** Use **.deb** for Ubuntu/Debian, **.rpm** for Fedora/RHEL, or **.AppImage** if unsure (works everywhere).

<details>
<summary>Troubleshooting AppImage on Linux</summary>

If the AppImage segfaults on launch, install **libfuse2** (`sudo apt install libfuse2` / `yay -S libfuse2` / `sudo dnf install fuse-libs`), or bypass FUSE entirely:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 ./Donut_0.19.0_amd64.AppImage
```

If that gives an EGL display error, try adding `WEBKIT_DISABLE_DMABUF_RENDERER=1` or `GDK_BACKEND=x11` to the command above. If issues persist, the **.deb** / **.rpm** packages are a more reliable alternative.

</details>

## What's different from the original Donut Browser?

This fork is based on [zhom/donutbrowser](https://github.com/zhom/donutbrowser) with the following fixes:

### Proxy fixes
- **HTTPS traffic now actually routed through proxy** (was going direct due to `Proxy::http()` instead of `Proxy::all()`)
- **TCP keepalive (30s)** on tunnel connections to prevent NAT/firewall idle drops
- **Connect timeout (10s) + request timeout (120s)** to prevent hanging connections
- **SOCKS4 domain name resolution** fixed (was failing on non-IP hostnames)
- **Connection pooling** with TCP keepalive on reqwest clients

### Unlocked features
- **Cross-OS fingerprints**: Generate Windows/macOS fingerprints from Linux (and vice versa)
- **All pro features enabled**: Extensions, cookie copy, synchronizer, advanced fingerprint editing

### Other
- Debug logging cleaned up (`log::error!` spam reduced to `log::debug!`)

## Features

- Unlimited isolated browser profiles with anti-detect fingerprints, powered by [Camoufox](https://camoufox.com)
- Cross-OS fingerprint generation (Windows, macOS, Linux)
- Proxy support per profile (HTTP, HTTPS, SOCKS4, SOCKS5) with authentication
- Cookie import/export (JSON + Netscape format)
- REST API for automation (Puppeteer, Playwright, Selenium compatible)
- VPN support (OpenVPN, WireGuard)
- Extension management
- Profile import from existing browsers

## License

AGPL-3.0 - see [LICENSE](LICENSE).

Based on [Donut Browser](https://github.com/zhom/donutbrowser) by zhom.
