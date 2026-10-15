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
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Appearance {
    /// "auto" | "dark" | "light"
    pub theme: String,
    /// "auto" (vraagt Hyprland) | "on" | "off"
    pub blur: String,
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
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: "auto".into(),
            blur: "auto".into(),
            font: "Inter".into(),
            accent: "#0a84ff".into(),
            radius: 22,
            width: 760,
            top_margin: 140,
            opacity: 0.55,
            fallback_opacity: 0.93,
            animation_ms: 160,
            max_height: 440,
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
            ignore: ["node_modules", "target", "__pycache__", ".git", ".cache", ".cargo", ".rustup"]
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
    pub icon_size: i32,
    pub extra_dirs: Vec<String>,
}

impl Default for Apps {
    fn default() -> Self {
        Self {
            columns: 6,
            icon_size: 48,
            extra_dirs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Calc {
    pub close_on_copy: bool,
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
