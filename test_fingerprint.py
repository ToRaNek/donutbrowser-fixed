#!/usr/bin/env python3
"""
Test fingerprint uniqueness using the full Donut Browser pipeline.

Uses donut-cli to generate real fingerprint configs (same pipeline as DonutBrowser),
then launches Camoufox with those CAMOU_CONFIG env vars and tests on fingerprint.com
via the Marionette protocol.
"""

import argparse
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time

CAMOU_EXE = os.path.join(
    os.environ.get("LOCALAPPDATA", ""),
    "DonutBrowserDev", "binaries", "camoufox", "v135.0.1-beta.24", "camoufox.exe"
)
CLI_EXE = os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "src-tauri", "target", "debug", "donut-cli.exe"
)
MARIONETTE_PORT = 2828
FP_URL = "https://demo.fingerprint.com/playground"


def build_cli():
    """Build donut-cli if needed."""
    if not os.path.exists(CLI_EXE):
        print("[build] Building donut-cli...")
        r = subprocess.run(
            ["cargo", "build", "--bin", "donut-cli"],
            cwd=os.path.join(os.path.dirname(os.path.abspath(__file__)), "src-tauri"),
            capture_output=True,
            timeout=600,
        )
        if r.returncode != 0:
            stderr_text = r.stderr.decode("utf-8", errors="replace") if r.stderr else ""
            print(f"[build] FAILED:\n{stderr_text[-500:]}")
            sys.exit(1)
        print("[build] OK")


def generate_config(target_os="macos"):
    """Generate fingerprint config via donut-cli. Returns (env_vars, config, target_os)."""
    print(f"  [step] Generating fingerprint config via donut-cli (os={target_os}) ...")
    r = subprocess.run(
        [CLI_EXE, "generate-fingerprint", "--os", target_os],
        capture_output=True,
        timeout=30,
    )
    if r.returncode != 0:
        stderr_text = r.stderr.decode("utf-8", errors="replace") if r.stderr else ""
        print(f"  [config] FAILED (exit {r.returncode}): {stderr_text[:300]}")
        return {}, {}, "unknown"

    stdout_text = r.stdout.decode("utf-8", errors="replace") if r.stdout else ""
    try:
        data = json.loads(stdout_text)
    except json.JSONDecodeError as exc:
        print(f"  [config] JSON parse error: {exc}")
        print(f"  stdout (first 300 chars): {stdout_text[:300]}")
        return {}, {}, "unknown"

    env_vars = data.get("env_vars", {})
    config = data.get("config", {})
    detected_os = data.get("target_os", target_os)

    # Print key fingerprint signals
    ua = config.get("navigator.userAgent", "N/A")
    hw = config.get("navigator.hardwareConcurrency", "N/A")
    sw = config.get("screen.width", "N/A")
    sh = config.get("screen.height", "N/A")
    iw = config.get("window.innerWidth", "N/A")
    ih = config.get("window.innerHeight", "N/A")
    aa = config.get("canvas:aaOffset", "N/A")
    fs = config.get("fonts:spacing_seed", "N/A")

    print(f"  [config] target_os: {detected_os}")
    print(f"  [config] UA: {ua}")
    print(f"  [config] hardware_concurrency: {hw}")
    print(f"  [config] screen: {sw}x{sh}, inner: {iw}x{ih}")
    print(f"  [config] canvas:aaOffset={aa}, fonts:spacing_seed={fs}")
    print(f"  [config] env_vars: {len(env_vars)} CAMOU_CONFIG chunks")

    return env_vars, config, detected_os


def write_anti_tamper_user_js(profile_dir, target_os="macos"):
    """Write anti-tampering Firefox prefs and font-hiding CSS, matching DonutBrowser."""
    user_js_path = os.path.join(profile_dir, "user.js")
    prefs = (
        '// Donut Browser anti-tampering overrides\n'
        'user_pref("ui.use_standins_for_native_colors", false);\n'
        'user_pref("gfx.color_management.mode", 2);\n'
        'user_pref("gfx.color_management.rendering_intent", 0);\n'
        'user_pref("focusmanager.testmode", false);\n'
        'user_pref("toolkit.cosmeticAnimations.enabled", true);\n'
        'user_pref("dom.input_events.security.minNumTicks", 5);\n'
        'user_pref("dom.input_events.security.minTimeElapsedInMS", 100);\n'
        'user_pref("dom.iframe_lazy_loading.enabled", true);\n'
        'user_pref("browser.cache.memory.enable", true);\n'
        'user_pref("privacy.partition.network_state", true);\n'
        'user_pref("network.dns.disablePrefetch", false);\n'
        'user_pref("network.dns.disablePrefetchFromHTTPS", false);\n'
        'user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", true);\n'
        'user_pref("javascript.options.use_fdlibm_for_sin_cos_tan", true);\n'
        'user_pref("webgl.sanitize-unmasked-renderer", false);\n'
    )
    with open(user_js_path, "w") as f:
        f.write(prefs)

    # Write font-hiding CSS on Windows when spoofing non-Windows OS
    if sys.platform == "win32" and target_os != "windows" and target_os != "win":
        chrome_dir = os.path.join(profile_dir, "chrome")
        os.makedirs(chrome_dir, exist_ok=True)
        css_path = os.path.join(chrome_dir, "userContent.css")
        css = (
            '/* Donut Browser: cross-OS font spoofing */\n'
            '/* Map Apple-specific font keywords to Trebuchet MS */\n'
            '@font-face { font-family: -apple-system-body; src: local("Trebuchet MS"); }\n'
            '@font-face { font-family: -apple-system; src: local("Trebuchet MS"); }\n'
            '@font-face { font-family: BlinkMacSystemFont; src: local("Trebuchet MS"); }\n'
            '/* Hide Windows-only fonts */\n'
            '@font-face { font-family: "HELV"; src: local("_"); }\n'
            '@font-face { font-family: "Small Fonts"; src: local("_"); }\n'
            '@font-face { font-family: "Segoe UI"; src: local("_"); }\n'
            '@font-face { font-family: "Calibri"; src: local("_"); }\n'
            '@font-face { font-family: "Cambria"; src: local("_"); }\n'
            '@font-face { font-family: "Consolas"; src: local("_"); }\n'
            '@font-face { font-family: "Constantia"; src: local("_"); }\n'
            '@font-face { font-family: "Corbel"; src: local("_"); }\n'
            '@font-face { font-family: "Ebrima"; src: local("_"); }\n'
            '@font-face { font-family: "Franklin Gothic Medium"; src: local("_"); }\n'
            '@font-face { font-family: "Gabriola"; src: local("_"); }\n'
            '@font-face { font-family: "Gadugi"; src: local("_"); }\n'
            '@font-face { font-family: "Ink Free"; src: local("_"); }\n'
            '@font-face { font-family: "Javanese Text"; src: local("_"); }\n'
            '@font-face { font-family: "Leelawadee UI"; src: local("_"); }\n'
            '@font-face { font-family: "Lucida Console"; src: local("_"); }\n'
            '@font-face { font-family: "MS Gothic"; src: local("_"); }\n'
            '@font-face { font-family: "MS PGothic"; src: local("_"); }\n'
            '@font-face { font-family: "Malgun Gothic"; src: local("_"); }\n'
            '@font-face { font-family: "Microsoft YaHei"; src: local("_"); }\n'
            '@font-face { font-family: "Nirmala UI"; src: local("_"); }\n'
            '@font-face { font-family: "Segoe UI Emoji"; src: local("_"); }\n'
            '@font-face { font-family: "Segoe UI Symbol"; src: local("_"); }\n'
            '@font-face { font-family: "SimSun"; src: local("_"); }\n'
            '@font-face { font-family: "Yu Gothic"; src: local("_"); }\n'
            '@font-face { font-family: "Webdings"; src: local("_"); }\n'
            '@font-face { font-family: "Wingdings"; src: local("_"); }\n'
            '@font-face { font-family: "Marlett"; src: local("_"); }\n'
        )
        with open(css_path, "w") as f:
            f.write(css)
        print(f"  [profile] Wrote font-hiding CSS for cross-OS spoofing")


def send_marionette(sock, cmd_id, method, params=None):
    """Send a Marionette command and return response."""
    cmd = json.dumps([0, cmd_id, method, params or {}])
    msg = f"{len(cmd)}:{cmd}"
    sock.send(msg.encode())
    time.sleep(1)
    data = b""
    while True:
        try:
            chunk = sock.recv(8192)
            if not chunk:
                break
            data += chunk
            text = data.decode("utf-8", errors="ignore")
            match = re.search(r'(\d+):(\[.+\])', text)
            if match:
                resp = json.loads(match.group(2))
                return resp
        except socket.timeout:
            break
    return None


def run_test(target_os="macos", run_index=0):
    """Run a single fingerprint test."""
    print(f"\n{'='*60}")
    print(f"  RUN {run_index + 1} (OS: {target_os})")
    print(f"{'='*60}")

    # 1. Generate config via donut-cli
    env_vars, config, detected_os = generate_config(target_os)
    if not env_vars:
        return {"visitor_id": "ERROR", "suspect_score": "ERROR", "tampering": "ERROR"}

    # 2. Create temp profile and write anti-tamper prefs
    profile_dir = tempfile.mkdtemp(prefix=f"donut_fp_test_{run_index}_")
    print(f"  [profile] {profile_dir}")
    write_anti_tamper_user_js(profile_dir, detected_os)

    # 3. Launch Camoufox with CAMOU_CONFIG env vars
    cmd = [CAMOU_EXE, "--marionette", "-profile", profile_dir, "-no-remote"]
    launch_env = os.environ.copy()
    launch_env.update(env_vars)

    proc = subprocess.Popen(cmd, env=launch_env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    print(f"  [launch] Camoufox PID={proc.pid}, {len(env_vars)} env vars injected")

    # Wait for Marionette to be ready
    time.sleep(8)

    result = {"visitor_id": "UNKNOWN", "suspect_score": "UNKNOWN", "tampering": "UNKNOWN"}

    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(10)
        sock.connect(("127.0.0.1", MARIONETTE_PORT))
        sock.recv(4096)  # greeting

        # Create session
        send_marionette(sock, 1, "WebDriver:NewSession", {"capabilities": {}})

        # Navigate to fingerprint.com
        send_marionette(sock, 2, "WebDriver:Navigate", {"url": FP_URL})

        # Wait for fingerprint scan to complete
        print("  [step] Waiting for fingerprint scan...")
        for attempt in range(20):
            time.sleep(3)
            resp = send_marionette(sock, attempt + 10, "WebDriver:ExecuteScript", {
                "script": "return document.body.innerText.indexOf('Suspect Score') > -1 ? 'READY' : 'LOADING'"
            })
            if resp and len(resp) > 3 and resp[3]:
                val = resp[3].get("value", "")
                if val == "READY":
                    print(f"  [step] Scan completed after ~{(attempt+1)*3}s")
                    break

        time.sleep(2)  # Extra time for DOM updates

        # Extract results in one script call
        resp = send_marionette(sock, 100, "WebDriver:ExecuteScript", {
            "script": """
                var t = document.body.innerText;
                var vid = 'UNKNOWN';
                var score = 'UNKNOWN';
                var tamp = 'UNKNOWN';

                // Find visitor ID (alphanumeric 20+ chars)
                var els = document.querySelectorAll('*');
                for (var i = 0; i < els.length; i++) {
                    var txt = els[i].textContent.trim();
                    if (/^[A-Za-z0-9]{20,}$/.test(txt) && els[i].children.length === 0) {
                        vid = txt;
                        break;
                    }
                }

                // Find suspect score
                var m = t.match(/Suspect Score[\\s\\t\\n]+(\\d+)/);
                if (m) score = m[1];

                // Find browser tampering - use JSON API response for accuracy
                var apiMatch = t.match(/"tampering":\s*(true|false)/);
                if (apiMatch) {
                    tamp = apiMatch[1] === 'false' ? 'Not detected' : 'Yes';
                } else if (t.indexOf('Browser Tampering') > -1) {
                    var idx = t.indexOf('Browser Tampering');
                    var after = t.substring(idx, idx + 40);
                    tamp = (after.indexOf('Yes') > -1) ? 'Yes' : 'Not detected';
                }

                return JSON.stringify({vid: vid, score: score, tamp: tamp});
            """
        })

        if resp and len(resp) > 3 and resp[3]:
            data = json.loads(resp[3].get("value", "{}"))
            result["visitor_id"] = data.get("vid", "UNKNOWN")
            result["suspect_score"] = data.get("score", "UNKNOWN")
            result["tampering"] = data.get("tamp", "UNKNOWN")

        sock.close()
    except Exception as e:
        print(f"  [error] {e}")

    # Cleanup
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()

    time.sleep(2)
    shutil.rmtree(profile_dir, ignore_errors=True)

    print(f"  [RESULT] visitor_id    = {result['visitor_id']}")
    print(f"  [RESULT] suspect_score = {result['suspect_score']}")
    print(f"  [RESULT] tampering     = {result['tampering']}")

    return result


def main():
    parser = argparse.ArgumentParser(
        description="Test Camoufox fingerprint uniqueness on fingerprint.com\n"
                    "Uses donut-cli to generate real configs (same pipeline as DonutBrowser)"
    )
    parser.add_argument("--count", "-n", type=int, default=2,
                        help="Number of profiles to test (default: 2)")
    parser.add_argument("--os", default="macos",
                        help="Target OS: macos, windows, linux (default: macos)")
    parser.add_argument("--generate-only", action="store_true",
                        help="Only generate and print configs, don't launch browsers")
    args = parser.parse_args()

    print(f"Camoufox fingerprint tester (powered by donut-cli)")
    print(f"Camoufox : {CAMOU_EXE}")
    print(f"donut-cli: {CLI_EXE}")
    print(f"Target OS: {args.os}")
    print(f"Runs     : {args.count}")

    # Auto-build donut-cli if needed
    build_cli()

    if not os.path.exists(CLI_EXE):
        print(f"\nERROR: donut-cli not found at {CLI_EXE}")
        print("Build it: cd src-tauri && cargo build --bin donut-cli")
        sys.exit(1)

    if args.generate_only:
        print(f"\n--- Generating {args.count} fingerprint configs ---")
        for i in range(args.count):
            print(f"\n{'='*60}")
            print(f"  CONFIG {i + 1}")
            print(f"{'='*60}")
            env_vars, config, detected_os = generate_config(args.os)
            print(f"  [result] {len(env_vars)} CAMOU_CONFIG chunks generated")
        return

    if not os.path.exists(CAMOU_EXE):
        print(f"\nERROR: Camoufox binary not found at {CAMOU_EXE}")
        sys.exit(1)

    results = []
    for i in range(args.count):
        r = run_test(args.os, i)
        results.append(r)

    # -- summary -------------------------------------------------------------
    print(f"\n{'='*60}")
    print("  SUMMARY")
    print(f"{'='*60}")

    for i, r in enumerate(results):
        print(f"  Run {i+1}: visitor_id={r['visitor_id']}  "
              f"score={r['suspect_score']}  tampering={r['tampering']}")

    vids = [r["visitor_id"] for r in results
            if r["visitor_id"] not in ("UNKNOWN", "ERROR", "TIMEOUT")]
    if len(vids) >= 2:
        unique = len(set(vids))
        if unique == len(vids):
            print(f"\n  >>> ALL {unique} visitor IDs are UNIQUE - fingerprints are different!")
        else:
            print(f"\n  >>> DUPLICATES FOUND: {unique}/{len(vids)} unique")
    elif len(vids) == 1:
        print(f"\n  >>> Only 1 valid visitor ID collected. Use --count 2+ to compare.")
    else:
        print(f"\n  >>> No valid visitor IDs collected. Check errors above.")

    tampered = [r for r in results if r["tampering"] == "Yes"]
    if tampered:
        print(f"  >>> Browser Tampering detected in {len(tampered)}/{len(results)} runs")
    else:
        print(f"  >>> No Browser Tampering detected")

    print()


if __name__ == "__main__":
    main()
