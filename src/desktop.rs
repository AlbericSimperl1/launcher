//! Parsen van .desktop-bestanden en het starten van apps.

use crate::util;
use gtk4::gio::{self, prelude::*};
use gtk4::prelude::DisplayExt;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub keywords: String,
    pub path: PathBuf,
}

fn app_dirs(extra: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let data_home = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| util::home().join(".local/share"));
    dirs.push(data_home.join("applications"));
    dirs.extend(extra.iter().cloned());
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    for d in data_dirs.split(':').filter(|d| !d.is_empty()) {
        dirs.push(Path::new(d).join("applications"));
    }
    dirs.push("/usr/share/applications".into());
    dirs.push("/var/lib/flatpak/exports/share/applications".into());
    dirs.push(util::home().join(".local/share/flatpak/exports/share/applications"));
    dirs
}

fn parse(path: &Path, id: String) -> Option<DesktopApp> {
    let text = std::fs::read_to_string(path).ok()?;
    let (mut name, mut icon, mut keywords, mut generic) = (None, String::new(), String::new(), String::new());
    let (mut typ, mut has_exec) = (String::new(), false);
    let (mut skip, mut only_show_in) = (false, String::new());
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim();
        match k.trim() {
            "Name" => name = Some(v.to_string()),
            "GenericName" => generic = v.to_string(),
            "Icon" => icon = v.to_string(),
            "Keywords" => keywords = v.replace(';', " "),
            "Type" => typ = v.to_string(),
            "Exec" => has_exec = true,
            "NoDisplay" | "Hidden" | "Terminal" if v.eq_ignore_ascii_case("true") => skip = true,
            "OnlyShowIn" => only_show_in = v.to_string(),
            _ => {}
        }
    }
    if skip || !has_exec || typ != "Application" {
        return None;
    }
    if !only_show_in.is_empty() {
        let current = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let matches = current
            .split(':')
            .any(|c| only_show_in.split(';').any(|o| o.eq_ignore_ascii_case(c)));
        if !matches {
            return None;
        }
    }
    Some(DesktopApp {
        id,
        name: name?,
        icon,
        keywords: format!("{generic} {keywords}"),
        path: path.to_path_buf(),
    })
}

pub fn load_apps(extra: &[PathBuf]) -> Vec<DesktopApp> {
    let mut seen = HashSet::new();
    let mut apps = Vec::new();
    for dir in app_dirs(extra) {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(app) = parse(&path, id) {
                apps.push(app);
            }
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

pub fn launch(app: &DesktopApp) {
    let Some(info) = gio::DesktopAppInfo::from_filename(&app.path) else {
        tracing::warn!("kan {} niet laden", app.path.display());
        return;
    };
    let ctx = gtk4::gdk::Display::default().map(|d| d.app_launch_context());
    let res = match &ctx {
        Some(c) => info.launch(&[], Some(c)),
        None => info.launch(&[], gio::AppLaunchContext::NONE),
    };
    if let Err(e) = res {
        tracing::warn!("start van {} mislukt: {e}", app.name);
    }
}
