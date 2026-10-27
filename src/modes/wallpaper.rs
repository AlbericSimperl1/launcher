//! Wallpaper-modus: thema-mappen scannen, filteren en zetten (standaard via awww).

use crate::config;
use crate::util::{self, Fuzzy};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Wall {
    pub path: PathBuf,
    pub name: String,
}

/// (thema = submap, wallpapers). Losse afbeeldingen in de hoofdmap hebben thema "".
pub type Section = (String, Vec<Wall>);

const EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif"];

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    v.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    v
}

fn images(dir: &Path) -> Vec<Wall> {
    sorted_entries(dir)
        .into_iter()
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| EXTS.contains(&e.to_lowercase().as_str()))
        })
        .map(|path| Wall {
            name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            path,
        })
        .collect()
}

pub fn scan(root: &Path) -> Vec<Section> {
    let mut out = vec![(String::new(), images(root))];
    for dir in sorted_entries(root).into_iter().filter(|p| p.is_dir()) {
        let theme = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if !theme.starts_with('.') {
            out.push((theme, images(&dir)));
        }
    }
    out.retain(|(_, w)| !w.is_empty());
    out
}

/// Fuzzy op "thema/naam", zodat typen van een themanaam dat hele thema toont.
pub fn filter(sections: &[Section], query: &str) -> Vec<Section> {
    let q = query.trim();
    if q.is_empty() {
        return sections.to_vec();
    }
    let mut fz = Fuzzy::new(q);
    sections
        .iter()
        .map(|(theme, walls)| {
            let hits = walls
                .iter()
                .filter(|w| fz.score(&format!("{theme}/{}", w.name)).is_some())
                .cloned()
                .collect();
            (theme.clone(), hits)
        })
        .filter(|(_, w): &Section| !w.is_empty())
        .collect()
}

/// Blokkeert kort als awww-daemon nog moet starten: aanroepen buiten de GTK-thread.
pub fn set(cfg: &config::Wallpaper, path: &Path) {
    let mut argv = util::split_cmd(&cfg.command);
    let daemon_up = || {
        Command::new("awww")
            .arg("query")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    // shortcut: vaste wachttijd na het starten van awww-daemon; bij een trage start mislukt de eerste keer.
    if argv.first().is_some_and(|p| p == "awww") && !daemon_up() {
        util::spawn(&["awww-daemon".into()]);
        std::thread::sleep(Duration::from_millis(400));
    }
    argv.push(path.to_string_lossy().into_owned());
    util::spawn(&argv);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall(name: &str) -> Wall {
        Wall {
            path: PathBuf::from(name),
            name: name.into(),
        }
    }

    #[test]
    fn filter_by_theme_and_name() {
        let s = vec![
            ("anime".to_string(), vec![wall("mk1.jpg"), wall("tnk1.jpg")]),
            ("kpop".to_string(), vec![wall("a.png")]),
        ];
        assert_eq!(filter(&s, "").len(), 2);
        let anime = filter(&s, "anime");
        assert_eq!(anime.len(), 1);
        assert_eq!(anime[0].1.len(), 2);
        let tnk = filter(&s, "tnk");
        assert_eq!(tnk[0].1[0].name, "tnk1.jpg");
    }
}
