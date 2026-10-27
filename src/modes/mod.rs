//! Modi van de launcher en de gedeelde resultaat-types.

pub mod apps;
pub mod browser;
pub mod calc;
pub mod files;
pub mod wallpaper;

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Apps,
    Files,
    Calc,
    Browser,
    Wallpaper,
}

impl Mode {
    /// Volgorde van de knoppen naast de zoekbalk.
    pub const BAR: [Mode; 4] = [Mode::Files, Mode::Calc, Mode::Browser, Mode::Apps];

    pub fn from_flag(c: char) -> Option<Mode> {
        match c {
            'f' => Some(Mode::Files),
            'c' => Some(Mode::Calc),
            'b' => Some(Mode::Browser),
            'w' => Some(Mode::Wallpaper),
            _ => None,
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Mode::Apps => "layout-grid",
            Mode::Files => "folder",
            Mode::Calc => "calculator",
            Mode::Browser => "globe",
            Mode::Wallpaper => "image",
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Mode::Apps => "Apps",
            Mode::Files => "Bestanden  (f)",
            Mode::Calc => "Calculator  (c)",
            Mode::Browser => "Browser  (b)",
            Mode::Wallpaper => "Wallpapers  (w)",
        }
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            Mode::Apps => "Find an app...",
            Mode::Files => "Find folders and files",
            Mode::Calc => "Calculate...",
            Mode::Browser => "Browse the web or bookmarks...",
            Mode::Wallpaper => "Find a wallpaper...",
        }
    }
}

#[derive(Debug, Clone)]
pub enum RowAction {
    Path { path: PathBuf, is_dir: bool },
    Url(String),
    Search(String),
}

#[derive(Debug, Clone)]
pub struct ListRow {
    pub icon: &'static str,
    pub title: String,
    pub subtitle: String,
    pub action: RowAction,
}
