//! Persistente state (pins, verborgen apps) en sessie-state (ans, geschiedenis).

use crate::util;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub pinned: Vec<String>,
    pub hidden: Vec<String>,
    #[serde(skip)]
    pub ans: Option<f64>,
    #[serde(skip)]
    pub history: Vec<String>,
}

fn path() -> PathBuf {
    util::config_dir().join("state.json")
}

impl State {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&p, json) {
                    tracing::warn!("state opslaan mislukt: {e}");
                }
            }
            Err(e) => tracing::warn!("state serialiseren mislukt: {e}"),
        }
    }

    pub fn is_pinned(&self, id: &str) -> bool {
        self.pinned.iter().any(|p| p == id)
    }

    pub fn is_hidden(&self, id: &str) -> bool {
        self.hidden.iter().any(|p| p == id)
    }

    pub fn pin(&mut self, id: &str) {
        if !self.is_pinned(id) {
            self.pinned.push(id.to_string());
        }
        self.save();
    }

    pub fn unpin(&mut self, id: &str) {
        self.pinned.retain(|p| p != id);
        self.save();
    }

    pub fn hide(&mut self, id: &str) {
        self.pinned.retain(|p| p != id);
        if !self.is_hidden(id) {
            self.hidden.push(id.to_string());
        }
        self.save();
    }

    pub fn unhide_all(&mut self) {
        self.hidden.clear();
        self.save();
    }
}
