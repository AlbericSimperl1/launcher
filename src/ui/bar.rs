//! Zoekbalk met leading-icoon en detectie van vlaggen (f / c / b + spatie).

use crate::modes::Mode;
use crate::ui::icons;
use gtk::prelude::*;
use gtk4 as gtk;
use std::rc::Rc;

pub struct Bar {
    pub root: gtk::Box,
    pub entry: gtk::Entry,
    lead: gtk::Image,
    crumb: gtk::Label,
}

impl Bar {
    pub fn new(_on_mode: Rc<dyn Fn(Mode)>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        root.add_css_class("search-row");

        let lead = icons::image("search", "dim", 18);
        lead.set_visible(false);
        root.append(&lead);

        let crumb = gtk::Label::new(None);
        crumb.add_css_class("crumb");
        crumb.set_ellipsize(gtk::pango::EllipsizeMode::Start);
        crumb.set_max_width_chars(30);
        crumb.set_visible(false);
        root.append(&crumb);

        let entry = gtk::Entry::new();
        entry.add_css_class("search");
        entry.set_hexpand(true);
        entry.set_placeholder_text(Some("Find whatever you want..."));
        root.append(&entry);

        Self {
            root,
            entry,
            lead,
            crumb,
        }
    }

    pub fn set_mode(&self, mode: Option<Mode>) {
        let lead_icon = match mode {
            Some(Mode::Files) => Some("search"),
            Some(Mode::Browser) => Some("globe"),
            Some(Mode::Calc) => Some("plus"),
            _ => None,
        };
        match lead_icon {
            Some(name) => {
                self.lead
                    .set_icon_name(Some(&icons::icon_name(name, "dim")));
                self.lead.set_visible(true);
            }
            None => self.lead.set_visible(false),
        }
        let placeholder = mode.map_or("Find whatever...", |m| m.placeholder());
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
