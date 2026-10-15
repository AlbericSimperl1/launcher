//! Browser-modus: URL-detectie, zoek-URL's en openen in een nieuw tabblad.

use crate::config::Browser;
use crate::util;

/// Geeft een genormaliseerde URL terug als de invoer op een URL lijkt.
pub fn looks_like_url(input: &str) -> Option<String> {
    let s = input.trim();
    if s.is_empty() || s.chars().any(char::is_whitespace) {
        return None;
    }
    for scheme in ["http://", "https://", "file://", "ftp://", "about:"] {
        if s.starts_with(scheme) {
            return Some(s.to_string());
        }
    }
    let host = s.split(['/', '?', '#']).next().unwrap_or(s);
    let host_no_port = host.split(':').next().unwrap_or(host);
    if host_no_port == "localhost" {
        return Some(format!("http://{s}"));
    }
    let is_ipv4 = host_no_port.split('.').count() == 4
        && host_no_port.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    if is_ipv4 {
        return Some(format!("http://{s}"));
    }
    let labels: Vec<&str> = host_no_port.split('.').collect();
    if labels.len() >= 2
        && labels.iter().all(|l| !l.is_empty())
        && labels.last().is_some_and(|t| t.len() >= 2 && t.chars().all(|c| c.is_ascii_alphabetic()))
    {
        return Some(format!("https://{s}"));
    }
    None
}

pub fn search_url(template: &str, query: &str) -> String {
    let enc = util::percent_encode(query.trim());
    if template.contains("{}") {
        template.replace("{}", &enc)
    } else {
        format!("{template}{enc}")
    }
}

pub fn open_url(cfg: &Browser, url: &str) {
    if cfg.command.trim().is_empty() {
        util::xdg_open(url);
    } else {
        let mut argv = util::split_cmd(&cfg.command);
        argv.push(url.to_string());
        util::spawn(&argv);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(looks_like_url("github.com/x").as_deref(), Some("https://github.com/x"));
        assert_eq!(looks_like_url("localhost:8080").as_deref(), Some("http://localhost:8080"));
        assert!(looks_like_url("hello world").is_none());
        assert!(looks_like_url("rust").is_none());
    }
}
