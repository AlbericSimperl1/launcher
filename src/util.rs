//! Kleine hulpfuncties: paden, logging, processen, fuzzy matching, klembord.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tracing_subscriber::EnvFilter;

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

pub fn expand_tilde(s: &str) -> PathBuf {
    if s == "~" {
        home()
    } else if let Some(rest) = s.strip_prefix("~/") {
        home().join(rest)
    } else {
        PathBuf::from(s)
    }
}

pub fn pretty_path(p: &Path) -> String {
    match p.strip_prefix(home()) {
        Ok(r) if r.as_os_str().is_empty() => "~".into(),
        Ok(r) => format!("~/{}", r.display()),
        Err(_) => p.display().to_string(),
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| home().join(".config"))
        .join("rust-launcher")
}

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| home().join(".cache"))
        .join("rust-launcher")
}

pub fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let dir = dirs::state_dir()
        .unwrap_or_else(|| home().join(".local/state"))
        .join("rust-launcher");
    std::fs::create_dir_all(&dir).ok()?;
    let file = tracing_appender::rolling::never(&dir, "launcher.log");
    let (writer, guard) = tracing_appender::non_blocking(file);
    let filter = EnvFilter::try_from_env("RL_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(filter)
        .init();
    Some(guard)
}

/// Splitst een commandoregel op spaties, met ondersteuning voor quotes.
pub fn split_cmd(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"') | (None, '\'') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() || started {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if !cur.is_empty() || started {
        out.push(cur);
    }
    out
}

/// Start een proces los van de launcher en ruimt het zombie-proces later op.
// src/util.rs — vervang spawn()
use std::os::unix::process::CommandExt;

pub fn spawn(argv: &[String]) -> bool {
    let Some((prog, args)) = argv.split_first() else {
        return false;
    };
    match Command::new(prog)
        .args(args)
        .env_remove("LD_PRELOAD")
        .process_group(0) // los van de launcher, overleeft herstart van de daemon
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(e) => {
            tracing::warn!("kan '{prog}' niet starten: {e}");
            false
        }
    }
}

pub fn xdg_open(target: &str) {
    spawn(&["xdg-open".to_string(), target.to_string()]);
}

pub fn copy_to_clipboard(text: &str) {
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::prelude::DisplayExt::clipboard(&display).set_text(text);
    }
}

pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Eén commando naar Hyprlands IPC-socket (wat `hyprctl` doet, zonder een proces te starten).
fn hypr_ipc(cmd: &str) -> Option<String> {
    use std::io::{Read, Write};
    let dir = std::env::var_os("XDG_RUNTIME_DIR")?;
    let sig = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let path = Path::new(&dir).join("hypr").join(sig).join(".socket.sock");
    let mut sock = std::os::unix::net::UnixStream::connect(path).ok()?;
    sock.set_read_timeout(Some(std::time::Duration::from_millis(300)))
        .ok()?;
    sock.write_all(cmd.as_bytes()).ok()?;
    let mut out = String::new();
    sock.read_to_string(&mut out).ok()?;
    Some(out)
}

/// Huidige blur-instelling van Hyprland als (size, passes).
pub fn hypr_blur_get() -> Option<(i64, i64)> {
    let get = |opt: &str| -> Option<i64> {
        let out = hypr_ipc(&format!("j/getoption decoration:blur:{opt}"))?;
        serde_json::from_str::<serde_json::Value>(&out).ok()?.get("int")?.as_i64()
    };
    Some((get("size")?, get("passes")?))
}

pub fn hypr_blur_set(size: i64, passes: i64) {
    let cmd = format!(
        "eval hl.config({{ decoration = {{ blur = {{ size = {size}, passes = {passes} }} }} }})"
    );
    let out = hypr_ipc(&cmd);
    if out.as_deref().map(str::trim) != Some("ok") {
        tracing::warn!("blur instellen mislukt: {out:?}");
    }
}

/// Fuzzy matcher rond nucleo (hergebruikt buffers tussen aanroepen).
pub struct Fuzzy {
    pat: Pattern,
    matcher: Matcher,
    buf: Vec<char>,
}

impl Fuzzy {
    pub fn new(query: &str) -> Self {
        Self {
            pat: Pattern::parse(query, CaseMatching::Smart, Normalization::Smart),
            matcher: Matcher::new(Config::DEFAULT),
            buf: Vec::new(),
        }
    }

    pub fn score(&mut self, haystack: &str) -> Option<u32> {
        self.pat
            .score(Utf32Str::new(haystack, &mut self.buf), &mut self.matcher)
    }
}
