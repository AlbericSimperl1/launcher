use crate::util;
use rusqlite::Connection;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

const REL: &[&str] = &[
    ".mozilla/firefox",
    ".config/mozilla/firefox",
    ".librewolf",
    ".config/librewolf",
    ".zen",
    ".config/zen",
    ".floorp",
    ".config/floorp",
    ".waterfox",
    "snap/firefox/common/.mozilla/firefox",
    ".config/chromium",
    ".config/google-chrome",
    ".config/google-chrome-beta",
    ".config/BraveSoftware/Brave-Browser",
    ".config/vivaldi",
    ".config/microsoft-edge",
    ".config/thorium",
    "snap/chromium/common/chromium",
];

pub fn load(path_override: &str) -> Vec<Bookmark> {
    let o = path_override.trim();
    let files: Vec<PathBuf> = if o.is_empty() {
        default_roots().iter().flat_map(|r| in_dir(r)).collect()
    } else {
        let p = util::expand_tilde(o);
        if p.is_dir() {
            in_dir(&p)
        } else {
            vec![p]
        }
    };
    let mut seen_files = HashSet::new();
    let mut all = Vec::new();
    for f in files {
        if seen_files.insert(f.clone()) {
            all.extend(load_path(&f));
        }
    }
    let mut seen = HashSet::new();
    all.retain(|b| seen.insert(b.url.clone()));
    all.sort_by_key(|b| b.title.to_lowercase());
    all
}

fn default_roots() -> Vec<PathBuf> {
    let h = util::home();
    let cfg = dirs::config_dir().unwrap_or_else(|| h.join(".config"));
    let mut out = Vec::new();
    for r in REL {
        out.push(h.join(r));
        if let Some(rest) = r.strip_prefix(".config/") {
            out.push(cfg.join(rest));
        }
    }
    if let Ok(rd) = std::fs::read_dir(h.join(".var/app")) {
        for app in rd.flatten() {
            for r in REL {
                out.push(app.path().join(r.replacen(".config/", "config/", 1)));
            }
        }
    }
    out
}

fn in_dir(d: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![d.to_path_buf()];
    if let Ok(rd) = std::fs::read_dir(d) {
        dirs.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
    }
    let mut v = Vec::new();
    for x in dirs {
        for f in ["places.sqlite", "Bookmarks"] {
            let p = x.join(f);
            if p.is_file() {
                v.push(p);
            }
        }
    }
    v
}

fn is_sqlite(p: &Path) -> bool {
    let mut h = [0u8; 15];
    std::fs::File::open(p)
        .and_then(|mut f| f.read_exact(&mut h))
        .is_ok()
        && &h == b"SQLite format 3"
}

fn load_path(p: &Path) -> Vec<Bookmark> {
    let res = if is_sqlite(p) {
        read_firefox(p)
    } else {
        read_chromium(p)
    };
    res.unwrap_or_else(|e| {
        tracing::warn!("bookmarks lezen uit {} mislukt: {e}", p.display());
        Vec::new()
    })
}

fn read_firefox(db: &Path) -> Result<Vec<Bookmark>, String> {
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
        let rows = stmt.query_map([], |r| {
            Ok(Bookmark {
                title: r.get(0)?,
                url: r.get(1)?,
            })
        })?;
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
                let title = node
                    .get("name")
                    .and_then(|n| n.as_str())
                    .filter(|n| !n.is_empty())
                    .unwrap_or(url);
                out.push(Bookmark {
                    title: title.to_string(),
                    url: url.to_string(),
                });
            }
        }
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for c in children {
            walk(c, out);
        }
    }
}
