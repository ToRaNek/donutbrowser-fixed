use crate::fingerprint_data::fonts;
use crate::fingerprint_data::webgl;

/// Build the anti-fingerprint JavaScript injection script for a given OS and seed.
pub fn build_injection_script(target_os: &str, seed: u32) -> String {
  let font_list = fonts::get_fonts_for_os(target_os);
  let webgl_profile = webgl::get_random_webgl_profile(target_os, seed);

  // Get host OS fonts to know which ones to HIDE
  let host_os = if cfg!(target_os = "windows") {
    "windows"
  } else if cfg!(target_os = "macos") {
    "macos"
  } else {
    "linux"
  };
  let host_fonts = fonts::get_fonts_for_os(host_os);

  // Fonts that should appear installed (target OS fonts)
  let target_json: Vec<String> = font_list.iter().map(|f| format!("\"{}\"", f)).collect();
  let target_array = target_json.join(",");

  // Fonts that should appear NOT installed (host-only fonts not in target list)
  let target_set: std::collections::HashSet<&&str> = font_list.iter().collect();
  let hide_fonts: Vec<&&str> = host_fonts
    .iter()
    .filter(|f| !target_set.contains(f))
    .collect();
  let hide_json: Vec<String> = hide_fonts.iter().map(|f| format!("\"{}\"", f)).collect();
  let hide_array = hide_json.join(",");

  format!(
    r#"(function() {{
  'use strict';

  console.log('[DonutBrowser] Anti-fingerprint injection active for OS:', '{target_os_name}');

  // === 1. Native function toString spoofing ===
  const _origToString = Function.prototype.toString;
  const _nativeFns = new Map();
  Function.prototype.toString = function() {{
    if (_nativeFns.has(this)) return _nativeFns.get(this);
    return _origToString.call(this);
  }};
  _nativeFns.set(Function.prototype.toString, 'function toString() {{ [native code] }}');
  function _markNative(fn, name) {{
    _nativeFns.set(fn, 'function ' + name + '() {{ [native code] }}');
  }}

  // === 2. Font spoofing ===
  const TARGET_FONTS = new Set([{target_fonts}]);
  const HIDE_FONTS = new Set([{hide_fonts}]);
  const GENERIC = new Set(['serif','sans-serif','monospace','cursive','fantasy','system-ui',
    'ui-serif','ui-sans-serif','ui-monospace','ui-rounded','-apple-system','BlinkMacSystemFont']);

  // Simple hash for deterministic per-font width offsets
  function _fontHash(name) {{
    let h = 0;
    for (let i = 0; i < name.length; i++) {{
      h = ((h << 5) - h + name.charCodeAt(i)) | 0;
    }}
    return h;
  }}

  function _extractFontName(cssFont) {{
    return cssFont.replace(/^\s*[\d.]+(px|pt|em|rem|%|vw|vh|ex|ch|cap|ic|lh|rlh|vi|vb|vmin|vmax)\s+/i, '')
      .replace(/^['"]|['"]$/g, '').trim();
  }}

  // Override document.fonts.check() - return true for target fonts, false for host-only
  if (typeof FontFaceSet !== 'undefined' && FontFaceSet.prototype.check) {{
    const _origCheck = FontFaceSet.prototype.check;
    FontFaceSet.prototype.check = function(font, text) {{
      try {{
        const name = _extractFontName(font.split(',')[0]);
        if (HIDE_FONTS.has(name)) return false;
        if (TARGET_FONTS.has(name) || GENERIC.has(name)) return true;
      }} catch(e) {{}}
      return _origCheck.call(this, font, text || '');
    }};
    _markNative(FontFaceSet.prototype.check, 'check');
  }}

  // Override measureText - key technique for font detection bypass
  // For TARGET fonts not actually installed: return width slightly different from fallback (simulates installed)
  // For HIDE fonts that ARE installed: return exact fallback width (simulates not installed)
  const _origMeasure = CanvasRenderingContext2D.prototype.measureText;

  // Cache fallback widths to be consistent
  const _fallbackCache = new Map();

  CanvasRenderingContext2D.prototype.measureText = function(text) {{
    const fontStr = this.font || '10px sans-serif';
    try {{
      const parts = fontStr.match(/^(.*?\d+(?:\.\w+)?(?:px|pt|em|rem|%))?\s*(.*)$/);
      if (!parts) return _origMeasure.call(this, text);
      const sizePrefix = (parts[1] || '10px').trim();
      const familyStr = (parts[2] || 'sans-serif').trim();
      const families = familyStr.split(',').map(function(f) {{ return f.trim().replace(/^['"]|['"]$/g, ''); }});
      const primary = families[0];

      if (!primary || GENERIC.has(primary)) return _origMeasure.call(this, text);

      // Get fallback measurement
      const cacheKey = sizePrefix + '|' + text;
      if (!_fallbackCache.has(cacheKey)) {{
        const saved = this.font;
        this.font = sizePrefix + ' monospace';
        const mono = _origMeasure.call(this, text);
        this.font = sizePrefix + ' sans-serif';
        const sans = _origMeasure.call(this, text);
        this.font = sizePrefix + ' serif';
        const ser = _origMeasure.call(this, text);
        this.font = saved;
        _fallbackCache.set(cacheKey, {{ mono: mono.width, sans: sans.width, serif: ser.width }});
      }}
      const fb = _fallbackCache.get(cacheKey);

      if (HIDE_FONTS.has(primary)) {{
        // Font should NOT exist — return exact fallback width
        const saved = this.font;
        const fallback = families.length > 1 ? families[families.length - 1] : 'monospace';
        this.font = sizePrefix + ' ' + (GENERIC.has(fallback) ? fallback : 'monospace');
        const result = _origMeasure.call(this, text);
        this.font = saved;
        return result;
      }}

      if (TARGET_FONTS.has(primary)) {{
        // Font SHOULD exist — check if it actually does
        const actual = _origMeasure.call(this, text);
        const fallback = families.length > 1 ? families[families.length - 1] : 'monospace';
        let fbWidth = fb.mono;
        if (fallback === 'sans-serif') fbWidth = fb.sans;
        else if (fallback === 'serif') fbWidth = fb.serif;

        // If actual width equals fallback, font is NOT installed — fake it
        if (Math.abs(actual.width - fbWidth) < 0.5) {{
          const hash = _fontHash(primary);
          const offset = ((hash % 7) - 3) * 0.3 + ((hash % 13) - 6) * 0.1;
          const fakeWidth = actual.width + offset;
          // Create a fake TextMetrics-like object
          return {{
            width: fakeWidth,
            actualBoundingBoxLeft: actual.actualBoundingBoxLeft + offset * 0.3,
            actualBoundingBoxRight: actual.actualBoundingBoxRight + offset * 0.7,
            actualBoundingBoxAscent: actual.actualBoundingBoxAscent,
            actualBoundingBoxDescent: actual.actualBoundingBoxDescent,
            fontBoundingBoxAscent: actual.fontBoundingBoxAscent,
            fontBoundingBoxDescent: actual.fontBoundingBoxDescent,
            emHeightAscent: actual.emHeightAscent,
            emHeightDescent: actual.emHeightDescent,
            alphabeticBaseline: actual.alphabeticBaseline,
            hangingBaseline: actual.hangingBaseline,
            ideographicBaseline: actual.ideographicBaseline
          }};
        }}
        return actual;
      }}
    }} catch(e) {{}}
    return _origMeasure.call(this, text);
  }};
  _markNative(CanvasRenderingContext2D.prototype.measureText, 'measureText');

  // === 3. Inject @font-face CSS to make target OS fonts "available" ===
  // Map missing target fonts to similar installed fonts via CSS
  const FONT_MAPPINGS = {{
    // macOS fonts → Windows equivalents
    'Helvetica Neue': 'Arial',
    'Helvetica': 'Arial',
    'San Francisco': 'Segoe UI',
    '.AppleSystemUIFont': 'Segoe UI',
    'Lucida Grande': 'Lucida Sans Unicode',
    'Geneva': 'Verdana',
    'Monaco': 'Consolas',
    'Menlo': 'Consolas',
    'Avenir': 'Century Gothic',
    'Avenir Next': 'Century Gothic',
    'Futura': 'Century Gothic',
    'Optima': 'Candara',
    'Gill Sans': 'Calibri',
    'Baskerville': 'Georgia',
    'Didot': 'Bodoni MT',
    'Palatino': 'Palatino Linotype',
    'Hoefler Text': 'Georgia',
    'Cochin': 'Georgia',
    'Copperplate': 'Copperplate Gothic',
    // Windows fonts → macOS equivalents
    'Segoe UI': 'Helvetica Neue',
    'Calibri': 'Helvetica',
    'Consolas': 'Menlo',
    'Cambria': 'Georgia',
    'Tahoma': 'Geneva',
    'Trebuchet MS': 'Trebuchet MS',
    // Linux fonts → common equivalents
    'DejaVu Sans': 'Arial',
    'Liberation Sans': 'Arial',
    'Liberation Serif': 'Times New Roman',
    'Liberation Mono': 'Courier New',
    'Ubuntu': 'Arial',
    'Cantarell': 'Arial',
    'Noto Sans': 'Arial',
  }};

  function _injectFontCSS() {{
    try {{
      const existing = document.getElementById('_donut_font_override');
      if (existing) return;
      const style = document.createElement('style');
      style.id = '_donut_font_override';
      let css = '';
      for (const [target, fallback] of Object.entries(FONT_MAPPINGS)) {{
        if (TARGET_FONTS.has(target)) {{
          css += '@font-face {{ font-family: "' + target + '"; src: local("' + fallback + '"); font-weight: 100 1000; font-style: normal; font-display: swap; }}\n';
          css += '@font-face {{ font-family: "' + target + '"; src: local("' + fallback + '"); font-weight: 100 1000; font-style: italic; font-display: swap; }}\n';
        }}
      }}
      if (css) {{
        style.textContent = css;
        const parent = document.head || document.documentElement;
        if (parent) {{
          parent.insertBefore(style, parent.firstChild);
        }}
      }}
    }} catch(e) {{}}
  }}

  // Robust CSS injection: poll until DOM is ready
  const _cssInterval = setInterval(function() {{
    try {{
      if (document.getElementById('_donut_font_override')) {{
        clearInterval(_cssInterval);
        return;
      }}
      const parent = document.head || document.documentElement;
      if (parent) {{
        _injectFontCSS();
        if (document.getElementById('_donut_font_override')) {{
          clearInterval(_cssInterval);
        }}
      }}
    }} catch(e) {{}}
  }}, 5);
  setTimeout(function() {{ clearInterval(_cssInterval); }}, 10000);

  // === 4. WebGL vendor/renderer spoofing ===
  const WEBGL_VENDOR = '{webgl_vendor}';
  const WEBGL_RENDERER = '{webgl_renderer}';

  function patchWebGL(proto) {{
    if (!proto) return;
    const _origGetParam = proto.getParameter;
    proto.getParameter = function(param) {{
      if (param === 0x9245) return WEBGL_VENDOR;
      if (param === 0x9246) return WEBGL_RENDERER;
      return _origGetParam.call(this, param);
    }};
    _markNative(proto.getParameter, 'getParameter');
  }}

  if (typeof WebGLRenderingContext !== 'undefined') patchWebGL(WebGLRenderingContext.prototype);
  if (typeof WebGL2RenderingContext !== 'undefined') patchWebGL(WebGL2RenderingContext.prototype);

  // === 5. Clean up CDP / automation traces ===
  if (Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver')) {{
    Object.defineProperty(Navigator.prototype, 'webdriver', {{
      get: function() {{ return false; }},
      configurable: true,
      enumerable: true
    }});
    _markNative(Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver').get, 'get webdriver');
  }}

  try {{
    for (const key of Object.keys(window)) {{
      if (key.startsWith('cdc_') || key.startsWith('__cdc_')) delete window[key];
    }}
  }} catch(e) {{}}

}})();"#,
    target_os_name = target_os,
    target_fonts = target_array,
    hide_fonts = hide_array,
    webgl_vendor = webgl_profile.vendor.replace('\'', "\\'"),
    webgl_renderer = webgl_profile.renderer.replace('\'', "\\'"),
  )
}
