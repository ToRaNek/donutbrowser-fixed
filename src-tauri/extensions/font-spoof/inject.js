// Anti-fingerprint JS overrides - runs in MAIN world at document_start
(function() {
  'use strict';

  // === Native toString spoofing ===
  var _ot = Function.prototype.toString;
  var _nf = new Map();
  Function.prototype.toString = function() {
    return _nf.has(this) ? _nf.get(this) : _ot.call(this);
  };
  _nf.set(Function.prototype.toString, 'function toString() { [native code] }');
  function _mn(fn, n) { _nf.set(fn, 'function ' + n + '() { [native code] }'); }

  // === Font lists ===
  var TARGET = new Set([
    'Helvetica Neue','Helvetica','San Francisco','.AppleSystemUIFont','Lucida Grande',
    'Monaco','Menlo','Geneva','Avenir','Avenir Next','Futura','Optima','Gill Sans',
    'Baskerville','Hoefler Text','Cochin','Didot','Palatino','Apple Color Emoji',
    'Apple SD Gothic Neo','Courier','Courier New','Times','Times New Roman','Arial',
    'Georgia','Verdana','Trebuchet MS','Impact','Tahoma','Symbol','PT Mono','PT Sans',
    'PT Serif','Charter','Damascus','Rockwell','Marker Felt','Noteworthy','Papyrus',
    'Chalkboard SE','Skia','Phosphate','Savoye LET','Snell Roundhand','Zapfino','Copperplate'
  ]);
  var HIDE = new Set([
    'Segoe UI','Calibri','Cambria','Consolas','Constantia','Corbel','Ebrima',
    'Franklin Gothic Medium','Gabriola','Gadugi','Ink Free','Javanese Text',
    'Leelawadee UI','Lucida Console','MS Gothic','MS PGothic','Malgun Gothic',
    'Microsoft YaHei','Nirmala UI','Segoe UI Emoji','Segoe UI Symbol','SimSun',
    'Yu Gothic','Webdings','Wingdings','Segoe Print','Segoe Script',
    'Microsoft Sans Serif','Marlett','MV Boli','Myanmar Text'
  ]);
  var GEN = new Set(['serif','sans-serif','monospace','cursive','fantasy','system-ui',
    'ui-serif','ui-sans-serif','ui-monospace','ui-rounded','-apple-system','BlinkMacSystemFont']);

  function _extractName(f) {
    return f.replace(/^(?:(?:italic|oblique|normal|bold|bolder|lighter|[1-9]00)\s+)*[\d.]+(?:px|pt|em|rem|%|vw|vh|ex|ch|cm|mm|in|pc|q)\s*/i, '')
      .replace(/^['"]|['"]$/g, '').trim();
  }

  // === Override document.fonts.check ===
  // Use instance override (document.fonts) since FontFaceSet constructor
  // may not be available at document_start timing
  function _overrideFontsCheck() {
    if (!document.fonts || !document.fonts.check) return false;
    if (document.fonts._donutPatched) return true;
    var _oc = document.fonts.check.bind(document.fonts);
    document.fonts.check = function(font, text) {
      try {
        var n = _extractName(font.split(',')[0].trim());
        if (HIDE.has(n)) return false;
        if (TARGET.has(n) || GEN.has(n)) return true;
      } catch(e) {}
      return _oc(font, text || '');
    };
    _mn(document.fonts.check, 'check');
    document.fonts._donutPatched = true;
    // Also patch prototype if available
    try {
      var proto = Object.getPrototypeOf(document.fonts);
      if (proto && proto.check) {
        var _opc = proto.check;
        proto.check = function(font, text) {
          try {
            var n = _extractName(font.split(',')[0].trim());
            if (HIDE.has(n)) return false;
            if (TARGET.has(n) || GEN.has(n)) return true;
          } catch(e) {}
          return _opc.call(this, font, text || '');
        };
        _mn(proto.check, 'check');
      }
    } catch(e) {}
    return true;
  }
  // Try now and retry if needed
  if (!_overrideFontsCheck()) {
    var _fi = setInterval(function() { if (_overrideFontsCheck()) clearInterval(_fi); }, 1);
    setTimeout(function() { clearInterval(_fi); }, 5000);
  }

  // === Hide Windows fonts via offsetWidth/getBoundingClientRect override ===
  // Font detection creates spans with font-family:"TestFont",fallback and checks offsetWidth.
  // For HIDE fonts, we make the width equal to the fallback width (as if font isn't installed).
  function _getFontFromStyle(el) {
    try {
      var ff = el.style.fontFamily || '';
      if (!ff) return null;
      var first = ff.split(',')[0].trim().replace(/^['"]|['"]$/g, '');
      return first;
    } catch(e) { return null; }
  }

  // Override offsetWidth getter
  var _origOffsetWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth');
  if (_origOffsetWidth && _origOffsetWidth.get) {
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
      get: function() {
        var w = _origOffsetWidth.get.call(this);
        var font = _getFontFromStyle(this);
        if (font && HIDE.has(font)) {
          // Return fallback width: temporarily set font to just the fallback
          var ff = this.style.fontFamily;
          var parts = ff.split(',');
          if (parts.length > 1) {
            var fallback = parts[parts.length - 1].trim();
            this.style.fontFamily = fallback;
            var fbw = _origOffsetWidth.get.call(this);
            this.style.fontFamily = ff;
            return fbw;
          }
        }
        return w;
      },
      configurable: true,
      enumerable: true
    });
    _mn(Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth').get, 'get offsetWidth');
  }

  // Override getBoundingClientRect for same purpose
  var _origGetBCR = Element.prototype.getBoundingClientRect;
  Element.prototype.getBoundingClientRect = function() {
    var rect = _origGetBCR.call(this);
    var font = _getFontFromStyle(this);
    if (font && HIDE.has(font)) {
      var ff = this.style.fontFamily;
      var parts = ff.split(',');
      if (parts.length > 1) {
        var fallback = parts[parts.length - 1].trim();
        this.style.fontFamily = fallback;
        var fbRect = _origGetBCR.call(this);
        this.style.fontFamily = ff;
        return fbRect;
      }
    }
    return rect;
  };
  _mn(Element.prototype.getBoundingClientRect, 'getBoundingClientRect');

  // === Override queryLocalFonts to return only target OS fonts ===
  if (typeof window.queryLocalFonts === 'function') {
    var _origQLF = window.queryLocalFonts;
    window.queryLocalFonts = async function() {
      var fonts = await _origQLF.call(window);
      // Filter: keep only fonts that exist in TARGET set, remove HIDE fonts
      return fonts.filter(function(f) {
        if (HIDE.has(f.family)) return false;
        return true;
      });
    };
    _mn(window.queryLocalFonts, 'queryLocalFonts');
  }

  // === Override canvas measureText to hide fonts ===
  var _origMT = CanvasRenderingContext2D.prototype.measureText;
  CanvasRenderingContext2D.prototype.measureText = function(text) {
    var fontStr = this.font || '10px sans-serif';
    try {
      var first = fontStr.split(',')[0].trim();
      var name = _extractName(first);
      if (name && HIDE.has(name)) {
        var parts = fontStr.split(',');
        var fallback = parts.length > 1 ? parts[parts.length-1].trim() : 'monospace';
        var saved = this.font;
        this.font = fontStr.replace(new RegExp('["\']?' + name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '["\']?'), fallback);
        var result = _origMT.call(this, text);
        this.font = saved;
        return result;
      }
    } catch(e) {}
    return _origMT.call(this, text);
  };
  _mn(CanvasRenderingContext2D.prototype.measureText, 'measureText');

  // === WebGL spoofing ===
  function pG(p) {
    if (!p) return;
    var _g = p.getParameter;
    p.getParameter = function(x) {
      if (x === 0x9245) return 'Apple';
      if (x === 0x9246) return 'Apple M1';
      return _g.call(this, x);
    };
    _mn(p.getParameter, 'getParameter');
  }
  if (typeof WebGLRenderingContext !== 'undefined') pG(WebGLRenderingContext.prototype);
  if (typeof WebGL2RenderingContext !== 'undefined') pG(WebGL2RenderingContext.prototype);

  // === CDP cleanup ===
  Object.defineProperty(Navigator.prototype, 'webdriver', {
    get: function() { return false; }, configurable: true, enumerable: true
  });
  _mn(Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver').get, 'get webdriver');
  try { for (var k of Object.keys(window)) { if (k.startsWith('cdc_')) delete window[k]; } } catch(e) {}
})();
