//! Configuratie uit ~/.config/rust-launcher/config.toml.

use crate::util;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub appearance: Appearance,
    pub files: Files,
    pub browser: Browser,
    pub apps: Apps,
    pub calc: Calc,
    pub wallpaper: Wallpaper,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Appearance {
    /// "auto" | "dark" | "light"
    pub theme: String,
    /// "auto" (aan onder Hyprland) | "on" | "off"
    pub blur: String,
    /// Blur-straal (Hyprland `decoration:blur:size`) zolang de launcher open is; 0 = Hyprlands waarde.
    pub blur_radius: u32,
    /// Blur-passes (Hyprland `decoration:blur:passes`) zolang de launcher open is; 0 = Hyprlands waarde.
    pub blur_passes: u32,
    pub font: String,
    pub accent: String,
    pub radius: u32,
    pub width: i32,
    pub top_margin: i32,
    /// Dekking van de achtergrond wanneer blur actief is (0.0 - 1.0).
    pub opacity: f64,
    /// Dekking wanneer blur niet beschikbaar is.
    pub fallback_opacity: f64,
    pub animation_ms: u32,
    pub max_height: i32,
    pub border_width: u32,
    /// "center" | "sidebar" | "hover" (muis tegen linkerrand = sidebar, keybind = center)
    pub placement: String,
    pub sidebar_width: i32,
    /// Afstand van de sidebar tot de schermranden in px (bv. om ruimte te laten voor een bar).
    pub sidebar_margin_top: i32,
    pub sidebar_margin_bottom: i32,
    pub sidebar_margin_side: i32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: "auto".into(),
            blur: "auto".into(),
            blur_radius: 0,
            blur_passes: 0,
            font: "Inter".into(),
            accent: "#0a84ff".into(),
            radius: 6,
            width: 600,
            top_margin: 140,
            opacity: 0.55,
            fallback_opacity: 0.93,
            animation_ms: 160,
            max_height: 360,
            border_width: 1,
            placement: "center".into(),
            sidebar_width: 440,
            sidebar_margin_top: 8,
            sidebar_margin_bottom: 8,
            sidebar_margin_side: 8,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Files {
    pub start_dir: String,
    /// Leeg = $EDITOR. Terminal-editors (nvim, helix, ...) worden in `terminal` gestart.
    pub editor: String,
    /// Leeg = xdg-open. Bijv. "nautilus", "dolphin", "thunar".
    pub file_manager: String,
    pub terminal: String,
    pub show_hidden: bool,
    pub max_depth: usize,
    pub max_results: usize,
    pub max_index: usize,
    pub ignore: Vec<String>,
}

impl Default for Files {
    fn default() -> Self {
        Self {
            start_dir: "~".into(),
            editor: String::new(),
            file_manager: String::new(),
            terminal: "kitty".into(),
            show_hidden: false,
            max_depth: 6,
            max_results: 60,
            max_index: 200_000,
            ignore: [
                "node_modules",
                "target",
                "__pycache__",
                ".git",
                ".cache",
                ".cargo",
                ".rustup",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Browser {
    /// Leeg = xdg-open. Bijv. "firefox --new-tab".
    pub command: String,
    /// {} wordt vervangen door de zoekterm.
    pub search_url: String,
    /// Leeg = automatisch zoeken (Firefox places.sqlite, Chromium/Brave/Vivaldi Bookmarks).
    pub bookmarks_path: String,
}

impl Default for Browser {
    fn default() -> Self {
        Self {
            command: String::new(),
            search_url: "https://duckduckgo.com/?q={}".into(),
            bookmarks_path: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Apps {
    pub columns: usize,
    /// Icoongrootte in de centered launcher; `sidebar_icon_size` in de sidebar (grid én list).
    pub icon_size: i32,
    pub sidebar_icon_size: i32,
    pub extra_dirs: Vec<String>,
    /// "grid" | "list"
    pub layout: String,
    /// Namen onder de iconen in grid-mode (de tooltip toont de naam altijd).
    pub show_names: bool,
}

impl Default for Apps {
    fn default() -> Self {
        Self {
            columns: 6,
            icon_size: 40,
            sidebar_icon_size: 32,
            extra_dirs: Vec::new(),
            layout: "grid".into(),
            show_names: true,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Calc {
    pub close_on_copy: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Wallpaper {
    /// Map met wallpapers; submappen zijn thema's.
    pub dir: String,
    /// Het pad van de afbeelding wordt achteraan toegevoegd.
    pub command: String,
    pub columns: usize,
}

impl Default for Wallpaper {
    fn default() -> Self {
        Self {
            dir: "~/Pictures/wallpapers".into(),
            command: "awww img --transition-type fade --transition-duration 0.4 --transition-fps 60".into(),
            columns: 3,
        }
    }
}

/// Zet één waarde in config.toml; commentaar en opmaak van de rest blijven behouden.
/// Een onleesbaar bestand wordt niet overschreven.
pub fn set(section: &str, key: &str, value: impl Into<toml_edit::Value>) -> std::io::Result<()> {
    let path = util::config_dir().join("config.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let new = set_in(&text, section, key, value.into())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::create_dir_all(util::config_dir())?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, new)?;
    std::fs::rename(&tmp, &path)
}

fn set_in(
    text: &str,
    section: &str,
    key: &str,
    mut value: toml_edit::Value,
) -> Result<String, toml_edit::TomlError> {
    let mut doc: toml_edit::DocumentMut = text.parse()?;
    if !doc.contains_key(section) {
        doc[section] = toml_edit::table();
    }
    let item = &mut doc[section][key];
    if let Some(old) = item.as_value() {
        *value.decor_mut() = old.decor().clone();
    }
    *item = toml_edit::Item::Value(value);
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_keeps_comments() {
        let text = "# kop\n[appearance]\nwidth = 560 # breedte\n";
        let out = set_in(text, "appearance", "width", 600.into()).unwrap();
        assert_eq!(out, "# kop\n[appearance]\nwidth = 600 # breedte\n");
        let out = set_in(&out, "wallpaper", "dir", "~/w".into()).unwrap();
        assert!(out.contains("[wallpaper]\ndir = \"~/w\""), "{out}");
        assert!(set_in("[[kapot", "a", "b", 1.into()).is_err());
    }
}

pub fn load() -> Config {
    let path = util::config_dir().join("config.toml");
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
            tracing::error!("ongeldige config {}: {e}", path.display());
            Config::default()
        }),
        Err(_) => Config::default(),
    }
}
