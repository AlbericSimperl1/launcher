//! Bookmarks inlezen: Firefox-forks (places.sqlite) en Chromium-browsers (JSON).

use crate::util;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

pub fn load(path_override: &str) -> Vec<Bookmark> {
    let mut all = Vec::new();
    if !path_override.trim().is_empty() {
        let p = util::expand_tilde(path_override.trim());
        all.extend(load_path(&p));
    } else {
        for p in firefox_dbs() {
            all.extend(load_path(&p));
        }
        for p in chromium_files() {
            all.extend(load_path(&p));
        }
    }
    let mut seen = HashSet::new();
    all.retain(|b| seen.insert(b.url.clone()));
    all.sort_by_key(|b| b.title.to_lowercase());
    all
}

fn load_path(p: &Path) -> Vec<Bookmark> {
    let res = if p.extension().and_then(|e| e.to_str()) == Some("sqlite") {
        read_firefox(p)
    } else {
        read_chromium(p)
    };
    res.unwrap_or_else(|e| {
        tracing::warn!("bookmarks lezen uit {} mislukt: {e}", p.display());
        Vec::new()
    })
}

fn firefox_dbs() -> Vec<PathBuf> {
    let h = util::home();
    let roots = [
        h.join(".mozilla/firefox"),
        h.join(".config/mozilla/firefox"),
        h.join(".librewolf"),
        h.join(".zen"),
        h.join(".var/app/org.mozilla.firefox/.mozilla/firefox"),
    ];
    let mut found = Vec::new();
    for root in roots {
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        let best = rd
            .flatten()
            .map(|e| e.path().join("places.sqlite"))
            .filter_map(|p| std::fs::metadata(&p).and_then(|m| m.modified()).ok().map(|t| (t, p)))
            .max_by_key(|(t, _)| *t);
        if let Some((_, p)) = best {
            found.push(p);
        }
    }
    found
}

fn chromium_files() -> Vec<PathBuf> {
    let cfg = dirs::config_dir().unwrap_or_else(|| util::home().join(".config"));
    [
        "chromium",
        "google-chrome",
        "BraveSoftware/Brave-Browser",
        "vivaldi",
        "microsoft-edge",
        "thorium",
    ]
    .iter()
    .map(|d| cfg.join(d).join("Default/Bookmarks"))
    .filter(|p| p.exists())
    .collect()
}

fn read_firefox(db: &Path) -> Result<Vec<Bookmark>, String> {
    // Firefox houdt de database open; werk op een kopie (inclusief WAL) om locks te vermijden.
    let tmp = std::env::temp_dir().join(format!("rust-launcher-places-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let copy = tmp.join("places.sqlite");
    std::fs::copy(db, &copy).map_err(|e| e.to_string())?;
    let wal = db.with_file_name("places.sqlite-wal");
    if wal.exists() {
        let _ = std::fs::copy(&wal, tmp.join("places.sqlite-wal"));
    }
    let result = (|| -> Result<Vec<Bookmark>, rusqlite::Error> {
        let conn = Connection::open(&copy)?;
        let mut stmt = conn.prepare(
            "SELECT COALESCE(NULLIF(b.title, ''), NULLIF(p.title, ''), p.url), p.url \
             FROM moz_bookmarks b JOIN moz_places p ON b.fk = p.id \
             WHERE b.type = 1 AND p.url LIKE 'http%'",
        )?;
        let rows = stmt.query_map([], |r| Ok(Bookmark { title: r.get(0)?, url: r.get(1)? }))?;
        rows.collect()
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result.map_err(|e| e.to_string())
}

fn read_chromium(path: &Path) -> Result<Vec<Bookmark>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    if let Some(roots) = v.get("roots").and_then(|r| r.as_object()) {
        for node in roots.values() {
            walk(node, &mut out);
        }
    }
    Ok(out)
}

fn walk(node: &serde_json::Value, out: &mut Vec<Bookmark>) {
    if node.get("type").and_then(|t| t.as_str()) == Some("url") {
        if let Some(url) = node.get("url").and_then(|u| u.as_str()) {
            if url.starts_with("http") {
                let title = node.get("name").and_then(|n| n.as_str()).filter(|n| !n.is_empty()).unwrap_or(url);
                out.push(Bookmark { title: title.to_string(), url: url.to_string() });
            }
        }
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for c in children {
            walk(c, out);
        }
    }
}
