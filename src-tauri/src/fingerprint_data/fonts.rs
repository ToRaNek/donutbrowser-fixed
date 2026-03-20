/// Per-OS font lists for cross-OS anti-detect browser profiles.
///
/// When creating a profile that pretends to be a different OS (e.g., macOS on
/// a Windows host), the reported font list must match the target OS to avoid
/// fingerprint inconsistencies.
pub fn get_fonts_for_os(os: &str) -> Vec<&'static str> {
  match os {
    "windows" => windows_fonts(),
    "macos" => macos_fonts(),
    "linux" => linux_fonts(),
    _ => windows_fonts(), // default
  }
}

fn windows_fonts() -> Vec<&'static str> {
  vec![
    "Arial",
    "Calibri",
    "Cambria",
    "Comic Sans MS",
    "Consolas",
    "Constantia",
    "Corbel",
    "Courier New",
    "Ebrima",
    "Franklin Gothic Medium",
    "Gabriola",
    "Gadugi",
    "Georgia",
    "Impact",
    "Ink Free",
    "Javanese Text",
    "Leelawadee UI",
    "Lucida Console",
    "Lucida Sans Unicode",
    "MS Gothic",
    "MS PGothic",
    "MS UI Gothic",
    "MS Mincho",
    "MV Boli",
    "Malgun Gothic",
    "Marlett",
    "Microsoft Himalaya",
    "Microsoft JhengHei",
    "Microsoft New Tai Lue",
    "Microsoft PhagsPa",
    "Microsoft Sans Serif",
    "Microsoft Tai Le",
    "Microsoft YaHei",
    "Microsoft Yi Baiti",
    "MingLiU-ExtB",
    "Mongolian Baiti",
    "Myanmar Text",
    "Nirmala UI",
    "Palatino Linotype",
    "Segoe MDL2 Assets",
    "Segoe Print",
    "Segoe Script",
    "Segoe UI",
    "Segoe UI Emoji",
    "Segoe UI Historic",
    "Segoe UI Symbol",
    "SimSun",
    "Sitka",
    "Sylfaen",
    "Symbol",
    "Tahoma",
    "Times New Roman",
    "Trebuchet MS",
    "Verdana",
    "Webdings",
    "Wingdings",
    "Yu Gothic",
  ]
}

fn macos_fonts() -> Vec<&'static str> {
  vec![
    ".AppleSystemUIFont",
    "Apple Color Emoji",
    "Apple SD Gothic Neo",
    "AppleGothic",
    "Arial",
    "Avenir",
    "Avenir Next",
    "Baskerville",
    "Chalkboard SE",
    "Charter",
    "Cochin",
    "Copperplate",
    "Courier",
    "Courier New",
    "Damascus",
    "Didot",
    "Futura",
    "Geneva",
    "Georgia",
    "Gill Sans",
    "Helvetica",
    "Helvetica Neue",
    "Hiragino Kaku Gothic ProN",
    "Hiragino Mincho ProN",
    "Hoefler Text",
    "Impact",
    "Lucida Grande",
    "Marker Felt",
    "Menlo",
    "Monaco",
    "Noteworthy",
    "Optima",
    "Osaka",
    "PT Mono",
    "PT Sans",
    "PT Serif",
    "Palatino",
    "Papyrus",
    "Phosphate",
    "Rockwell",
    "San Francisco",
    "Savoye LET",
    "Skia",
    "Snell Roundhand",
    "Songti SC",
    "STIXGeneral",
    "Symbol",
    "Tahoma",
    "Times",
    "Times New Roman",
    "Trebuchet MS",
    "Verdana",
    "Zapfino",
  ]
}

fn linux_fonts() -> Vec<&'static str> {
  vec![
    "Liberation Mono",
    "Liberation Sans",
    "Liberation Serif",
    "DejaVu Sans",
    "DejaVu Sans Mono",
    "DejaVu Serif",
    "Ubuntu",
    "Ubuntu Mono",
    "Cantarell",
    "Noto Sans",
    "Noto Serif",
    "Noto Mono",
    "Droid Sans",
    "Droid Serif",
    "Droid Sans Mono",
    "FreeSans",
    "FreeMono",
    "FreeSerif",
    "Nimbus Sans",
    "Nimbus Roman",
    "Nimbus Mono",
    "URW Bookman",
    "URW Gothic",
    "Courier 10 Pitch",
    "Bitstream Vera Sans",
    "Bitstream Vera Serif",
    "Bitstream Vera Sans Mono",
  ]
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_fonts_for_known_os() {
    assert!(!get_fonts_for_os("windows").is_empty());
    assert!(!get_fonts_for_os("macos").is_empty());
    assert!(!get_fonts_for_os("linux").is_empty());
  }

  #[test]
  fn test_default_falls_back_to_windows() {
    assert_eq!(get_fonts_for_os("unknown"), get_fonts_for_os("windows"));
  }

  #[test]
  fn test_os_specific_fonts_present() {
    let win = get_fonts_for_os("windows");
    assert!(win.contains(&"Segoe UI"));
    assert!(win.contains(&"Calibri"));

    let mac = get_fonts_for_os("macos");
    assert!(mac.contains(&"Helvetica Neue"));
    assert!(mac.contains(&"San Francisco"));

    let linux = get_fonts_for_os("linux");
    assert!(linux.contains(&"DejaVu Sans"));
    assert!(linux.contains(&"Liberation Sans"));
  }
}
