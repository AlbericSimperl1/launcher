//! Lucide-achtige iconenset, runtime geïnstalleerd als scalable icons in de cache-map.

use crate::ui::theme;
use crate::util;
use gtk4 as gtk;

const ICONS: &[(&str, &str)] = &[
    ("folder", r#"<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>"#),
    ("file", r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/>"#),
    ("terminal", r#"<polyline points="4 17 10 11 4 5"/><line x1="12" x2="20" y1="19" y2="19"/>"#),
    ("calculator", r#"<rect width="16" height="20" x="4" y="2" rx="2"/><line x1="8" x2="16" y1="6" y2="6"/><line x1="16" x2="16" y1="14" y2="18"/><path d="M16 10h.01"/><path d="M12 10h.01"/><path d="M8 10h.01"/><path d="M12 14h.01"/><path d="M8 14h.01"/><path d="M12 18h.01"/><path d="M8 18h.01"/>"#),
    ("globe", r#"<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/>"#),
    ("layout-grid", r#"<rect width="7" height="7" x="3" y="3" rx="1"/><rect width="7" height="7" x="14" y="3" rx="1"/><rect width="7" height="7" x="14" y="14" rx="1"/><rect width="7" height="7" x="3" y="14" rx="1"/>"#),
    ("bookmark", r#"<path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"/>"#),
    ("image", r#"<rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>"#),
    ("search", r#"<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>"#),
];

/// Schrijft alle iconen in drie kleurvarianten (fg, dim, white) en registreert de map.
pub fn install(dark: bool) {
    let p = theme::palette(dark);
    let variants = [("fg", p.fg), ("dim", p.dim), ("white", "#ffffff")];
    let base = util::cache_dir().join("icons");
    let dir = base.join("hicolor/scalable/apps");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!("icon-map aanmaken mislukt: {e}");
        return;
    }
    for (name, body) in ICONS {
        for (variant, color) in variants {
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="{color}" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#
            );
            let _ = std::fs::write(dir.join(format!("rl-{name}-{variant}.svg")), svg);
        }
    }
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_search_path(&base);
    }
}

pub fn icon_name(name: &str, variant: &str) -> String {
    format!("rl-{name}-{variant}")
}

pub fn image(name: &str, variant: &str, size: i32) -> gtk::Image {
    let img = gtk::Image::from_icon_name(&icon_name(name, variant));
    img.set_pixel_size(size);
    img
}
