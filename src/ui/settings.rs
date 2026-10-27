//! Settingspanel: formulier dat elke wijziging direct in config.toml schrijft.
//! De config-watch van de launcher herlaadt daarna thema, plaatsing en de rest.

use crate::config::{self, Config};
use gtk::gdk;
use gtk::prelude::*;
use gtk4 as gtk;
use std::rc::Rc;

pub struct Settings {
    pub scroll: gtk::ScrolledWindow,
    content: gtk::Box,
}

fn save(section: &str, key: &str, value: impl Into<toml_edit::Value>) {
    if let Err(e) = config::set(section, key, value) {
        tracing::warn!("config {section}.{key} opslaan mislukt: {e}");
    }
}

fn row(content: &gtk::Box, label: &str, widget: &impl IsA<gtk::Widget>) {
    let r = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    r.add_css_class("setting");
    let l = gtk::Label::new(Some(label));
    l.set_xalign(0.0);
    l.set_hexpand(true);
    r.append(&l);
    widget.set_valign(gtk::Align::Center);
    r.append(widget);
    content.append(&r);
}

fn header(content: &gtk::Box, text: &str) {
    let l = gtk::Label::new(Some(text));
    l.add_css_class("section-title");
    l.set_xalign(0.0);
    content.append(&l);
}

fn choice(content: &gtk::Box, label: &str, sec: &'static str, key: &'static str, opts: &'static [&'static str], cur: &str) {
    let dd = gtk::DropDown::from_strings(opts);
    dd.set_selected(opts.iter().position(|o| *o == cur).unwrap_or(0) as u32);
    dd.connect_selected_notify(move |d| {
        if let Some(o) = opts.get(d.selected() as usize) {
            save(sec, key, *o);
        }
    });
    row(content, label, &dd);
}

/// `digits == 0` schrijft een integer, anders een float afgerond op `digits` decimalen.
#[allow(clippy::too_many_arguments)]
fn spin(content: &gtk::Box, label: &str, sec: &'static str, key: &'static str, min: f64, max: f64, step: f64, digits: u32, cur: f64) {
    let sb = gtk::SpinButton::with_range(min, max, step);
    sb.set_digits(digits);
    sb.set_value(cur);
    sb.connect_value_changed(move |s| {
        if digits == 0 {
            save(sec, key, s.value() as i64);
        } else {
            let f = 10f64.powi(digits as i32);
            save(sec, key, (s.value() * f).round() / f);
        }
    });
    row(content, label, &sb);
}

/// Schrijft bij Enter of focusverlies, niet per toetsaanslag.
fn text(content: &gtk::Box, label: &str, sec: &'static str, key: &'static str, cur: &str) -> gtk::Entry {
    let e = gtk::Entry::new();
    e.set_text(cur);
    e.set_width_chars(22);
    let last = Rc::new(std::cell::RefCell::new(cur.to_string()));
    let commit = Rc::new(move |e: &gtk::Entry| {
        let t = e.text().to_string();
        if *last.borrow() != t {
            save(sec, key, t.as_str());
            *last.borrow_mut() = t;
        }
    });
    let c = commit.clone();
    e.connect_activate(move |e| c(e));
    let focus = gtk::EventControllerFocus::new();
    let e2 = e.clone();
    focus.connect_leave(move |_| commit(&e2));
    e.add_controller(focus);
    row(content, label, &e);
    e
}

impl Settings {
    pub fn new(max_height: i32) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
        content.add_css_class("settings");
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&content));
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(max_height);
        scroll.set_overlay_scrolling(true);
        Self { scroll, content }
    }

    /// Bouwt het formulier opnieuw op met de huidige waarden (bij elke keer openen).
    pub fn build(&self, cfg: &Config) {
        let c = &self.content;
        while let Some(w) = c.first_child() {
            c.remove(&w);
        }
        let a = &cfg.appearance;

        header(c, "Uiterlijk");
        choice(c, "Thema", "appearance", "theme", &["auto", "dark", "light"], &a.theme);
        let accent = text(c, "Accentkleur", "appearance", "accent", &a.accent);
        let swatch = gtk::DrawingArea::new();
        swatch.set_content_width(18);
        swatch.set_content_height(18);
        let e = accent.clone();
        swatch.set_draw_func(move |_, cr, w, h| {
            if let Ok(rgba) = gdk::RGBA::parse(e.text().as_str()) {
                cr.set_source_rgba(rgba.red() as f64, rgba.green() as f64, rgba.blue() as f64, 1.0);
                cr.rectangle(0.0, 0.0, w as f64, h as f64);
                let _ = cr.fill();
            }
        });
        let s = swatch.clone();
        accent.connect_changed(move |_| s.queue_draw());
        if let Some(r) = accent.parent().and_downcast::<gtk::Box>() {
            r.insert_child_after(&swatch, r.first_child().as_ref());
        }
        spin(c, "Randdikte", "appearance", "border_width", 0.0, 10.0, 1.0, 0, a.border_width as f64);
        spin(c, "Hoekradius", "appearance", "radius", 0.0, 40.0, 1.0, 0, a.radius as f64);
        spin(c, "Dekking", "appearance", "opacity", 0.0, 1.0, 0.05, 2, a.opacity);
        let blur = gtk::Switch::new();
        blur.set_active(crate::ui::theme::resolve_blur(a));
        blur.connect_active_notify(|s| save("appearance", "blur", if s.is_active() { "on" } else { "off" }));
        row(c, "Blur", &blur);
        text(c, "Lettertype", "appearance", "font", &a.font);

        header(c, "Indeling");
        choice(c, "Plaatsing", "appearance", "placement", &["center", "sidebar", "hover"], &a.placement);
        choice(c, "Apps", "apps", "layout", &["grid", "list"], &cfg.apps.layout);
        spin(c, "Kolommen (grid)", "apps", "columns", 1.0, 12.0, 1.0, 0, cfg.apps.columns as f64);
        spin(c, "Breedte (center)", "appearance", "width", 300.0, 1600.0, 10.0, 0, a.width as f64);
        spin(c, "Max. hoogte (center)", "appearance", "max_height", 150.0, 1400.0, 10.0, 0, a.max_height as f64);
        spin(c, "Breedte (sidebar)", "appearance", "sidebar_width", 200.0, 1200.0, 10.0, 0, a.sidebar_width as f64);

        header(c, "Standaard apps");
        text(c, "Terminal", "files", "terminal", &cfg.files.terminal);
        text(c, "Editor", "files", "editor", &cfg.files.editor);
        text(c, "Bestandsbeheer", "files", "file_manager", &cfg.files.file_manager);
        text(c, "Browser", "browser", "command", &cfg.browser.command);

        header(c, "Wallpapers");
        text(c, "Map", "wallpaper", "dir", &cfg.wallpaper.dir);
        text(c, "Commando", "wallpaper", "command", &cfg.wallpaper.command);
        spin(c, "Kolommen", "wallpaper", "columns", 1.0, 8.0, 1.0, 0, cfg.wallpaper.columns as f64);

        let (_, nat, _, _) = c.measure(gtk::Orientation::Vertical, -1);
        self.scroll
            .set_min_content_height(nat.min(self.scroll.max_content_height()));
    }

    pub fn focus_first(&self) {
        self.content.child_focus(gtk::DirectionType::TabForward);
    }
}
