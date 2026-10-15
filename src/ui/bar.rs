//! Zoekbalk met mode-knoppen en detectie van vlaggen (f / c / b + spatie).

use crate::modes::Mode;
use crate::ui::icons;
use gtk4 as gtk;
use gtk::prelude::*;
use std::rc::Rc;

pub struct Bar {
    pub root: gtk::Box,
    pub entry: gtk::Entry,
    crumb: gtk::Label,
    buttons: Vec<(Mode, gtk::Button, gtk::Image)>,
}

impl Bar {
    pub fn new(on_mode: Rc<dyn Fn(Mode)>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        root.add_css_class("search-row");

        let search_icon = icons::image("search", "dim", 22);
        root.append(&search_icon);

        let crumb = gtk::Label::new(None);
        crumb.add_css_class("crumb");
        crumb.set_ellipsize(gtk::pango::EllipsizeMode::Start);
        crumb.set_max_width_chars(30);
        crumb.set_visible(false);
        root.append(&crumb);

        let entry = gtk::Entry::new();
        entry.add_css_class("search");
        entry.set_hexpand(true);
        entry.set_placeholder_text(Some("Zoek…  (f bestanden · c rekenen · b browser)"));
        root.append(&entry);

        let mut buttons = Vec::new();
        for mode in Mode::BAR {
            let btn = gtk::Button::new();
            btn.add_css_class("mode");
            btn.set_focusable(false);
            btn.set_focus_on_click(false);
            btn.set_tooltip_text(Some(mode.tooltip()));
            let img = icons::image(mode.icon(), "fg", 18);
            btn.set_child(Some(&img));
            let cb = on_mode.clone();
            btn.connect_clicked(move |_| cb(mode));
            root.append(&btn);
            buttons.push((mode, btn, img));
        }

        Self { root, entry, crumb, buttons }
    }

    pub fn set_mode(&self, mode: Option<Mode>) {
        for (m, btn, img) in &self.buttons {
            let active = Some(*m) == mode;
            if active {
                btn.add_css_class("active");
            } else {
                btn.remove_css_class("active");
            }
            img.set_icon_name(Some(&icons::icon_name(m.icon(), if active { "white" } else { "fg" })));
        }
        let placeholder = mode.map_or("Zoek…  (f bestanden · c rekenen · b browser)", |m| m.placeholder());
        self.entry.set_placeholder_text(Some(placeholder));
        if mode != Some(Mode::Files) {
            self.crumb.set_visible(false);
        }
    }

    pub fn set_crumb(&self, text: Option<&str>) {
        match text {
            Some(t) => {
                self.crumb.set_text(&format!("{t} ›"));
                self.crumb.set_visible(true);
            }
            None => self.crumb.set_visible(false),
        }
    }
}

/// Herkent `f `, `c ` of `b ` aan het begin van de invoer. Geeft de modus en de resterende tekst.
pub fn detect_flag(text: &str) -> Option<(Mode, String)> {
    let mut it = text.chars();
    let flag = it.next()?;
    if it.next()? != ' ' {
        return None;
    }
    let mode = Mode::from_flag(flag)?;
    Some((mode, text.chars().skip(2).collect()))
}
