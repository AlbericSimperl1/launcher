//! Bestanden-modus: indexeren, fuzzy zoeken en openen.

use crate::config::Files;
use crate::util::{self, Fuzzy};
use gtk4::gio;
use std::collections::VecDeque;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    /// Pad relatief ten opzichte van de map waarin geïndexeerd is.
    pub rel: String,
    pub is_dir: bool,
    pub is_exec: bool,
}

/// Breadth-first scan, zodat ondiepe resultaten eerst komen en `limit` eerlijk afkapt.
pub fn scan(
    root: &Path,
    hidden: bool,
    depth: usize,
    ignore: &[String],
    limit: usize,
) -> Vec<FileEntry> {
    let mut out = Vec::new();
    let mut queue = VecDeque::from([(root.to_path_buf(), 0usize)]);
    while let Some((dir, level)) = queue.pop_front() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            if out.len() >= limit {
                return out;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if (!hidden && name.starts_with('.')) || ignore.iter().any(|i| i == &name) {
                continue;
            }
            let path = entry.path();
            let Ok(ft) = entry.file_type() else { continue };
            let meta = if ft.is_symlink() {
                std::fs::metadata(&path).ok()
            } else {
                entry.metadata().ok()
            };
            let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
            let is_exec = !is_dir
                && meta
                    .as_ref()
                    .is_some_and(|m| m.permissions().mode() & 0o111 != 0);
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            if is_dir && !ft.is_symlink() && level + 1 < depth {
                queue.push_back((path.clone(), level + 1));
            }
            out.push(FileEntry {
                path,
                name,
                rel,
                is_dir,
                is_exec,
            });
        }
    }
    out
}

/// Directe kinderen van een map: mappen eerst, daarna alfabetisch.
pub fn children(dir: &Path, hidden: bool, ignore: &[String]) -> Vec<FileEntry> {
    let mut v = scan(dir, hidden, 1, ignore, 5_000);
    sort_entries(&mut v);
    v
}

fn sort_entries(v: &mut [FileEntry]) {
    v.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

pub fn filter(index: &[FileEntry], query: &str, max: usize) -> Vec<FileEntry> {
    let mut fz = Fuzzy::new(query);
    let mut scored: Vec<(u32, &FileEntry)> = index
        .iter()
        .filter_map(|e| {
            let by_name = fz.score(&e.name).map(|s| s.saturating_mul(2));
            let by_path = if e.rel.contains('/') {
                fz.score(&e.rel)
            } else {
                None
            };
            by_name.max(by_path).map(|s| (s, e))
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.rel.len().cmp(&b.1.rel.len()))
    });
    scored
        .into_iter()
        .take(max)
        .map(|(_, e)| e.clone())
        .collect()
}
const TUI_EDITORS: &[&str] = &[
    "nvim", "vim", "vi", "nano", "hx", "helix", "micro", "emacs", "kak", "nvi",
];
const TUI_MANAGERS: &[&str] = &["yazi", "ranger", "lf", "nnn", "vifm", "mc", "joshuto"];

fn is_text(path: &Path) -> bool {
    let mut buf = [0u8; 1024];
    let n = std::fs::File::open(path)
        .and_then(|mut f| f.read(&mut buf))
        .unwrap_or(0);
    let (ct, _) = gio::content_type_guess(Some(path), &buf[..n]);
    gio::content_type_is_a(&ct, "text/plain")
}

fn run(cfg: &Files, cmd: &str, tui: &[&str], target: &Path) -> bool {
    let mut argv = util::split_cmd(cmd);
    let prog = argv
        .first()
        .and_then(|p| Path::new(p).file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if tui.contains(&prog.as_str()) {
        let mut wrapped = util::split_cmd(&cfg.terminal);
        wrapped.push("-e".into());
        wrapped.append(&mut argv);
        argv = wrapped;
    }
    argv.push(target.to_string_lossy().into_owned());
    util::spawn(&argv)
}

fn editor_cmd(cfg: &Files) -> String {
    let e = cfg.editor.trim();
    if e.is_empty() {
        std::env::var("EDITOR").unwrap_or_default()
    } else {
        e.to_string()
    }
}

pub fn open_default(path: &Path) {
    util::xdg_open(&path.to_string_lossy());
}

pub fn open_primary(cfg: &Files, path: &Path) {
    if !cfg.editor.trim().is_empty() && is_text(path) {
        open_in_editor(cfg, path);
    } else {
        open_default(path);
    }
}

pub fn open_in_manager(cfg: &Files, path: &Path, is_dir: bool) {
    let target = if is_dir {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    if cfg.file_manager.trim().is_empty() || !run(cfg, &cfg.file_manager, TUI_MANAGERS, target) {
        util::xdg_open(&target.to_string_lossy());
    }
}

pub fn open_in_editor(cfg: &Files, path: &Path) {
    let editor = editor_cmd(cfg);
    if editor.trim().is_empty() || !run(cfg, &editor, TUI_EDITORS, path) {
        open_default(path);
    }
}
