//! Wallpaper-grid: per thema een kopje en een grid met thumbnails.

use crate::modes::wallpaper::Section;
use crate::ui::results::ensure_visible;
use gtk::gdk;
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

pub struct Walls {
    pub scroll: gtk::ScrolledWindow,
    content: gtk::Box,
    tiles: RefCell<Vec<gtk::Box>>,
    pictures: RefCell<HashMap<PathBuf, gtk::Picture>>,
}

impl Walls {
    pub fn new(max_height: i32) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&content));
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(max_height);
        scroll.set_focusable(false);
        scroll.set_overlay_scrolling(true);
        Self {
            scroll,
            content,
            tiles: RefCell::new(Vec::new()),
            pictures: RefCell::new(HashMap::new()),
        }
    }

    pub fn populate(
        &self,
        sections: &[Section],
        cols: usize,
        tile_w: i32,
        selected: usize,
        thumbs: &HashMap<PathBuf, gdk::Texture>,
        on_click: Rc<dyn Fn(usize)>,
    ) {
        while let Some(c) = self.content.first_child() {
            self.content.remove(&c);
        }
        let mut tiles = Vec::new();
        let mut pictures = HashMap::new();
        let (pic_w, pic_h) = (tile_w - 8, (tile_w - 8) * 9 / 16);
        for (theme, walls) in sections {
            let title = gtk::Label::new(Some(if theme.is_empty() { "overig" } else { theme }));
            title.add_css_class("section-title");
            title.set_xalign(0.0);
            self.content.append(&title);

            let grid = gtk::Grid::new();
            grid.set_column_homogeneous(true);
            grid.set_row_spacing(2);
            grid.set_column_spacing(2);
            grid.set_halign(gtk::Align::Start);
            for (i, wall) in walls.iter().enumerate() {
                let idx = tiles.len();
                let tile = gtk::Box::new(gtk::Orientation::Vertical, 4);
                tile.add_css_class("wall");
                tile.set_size_request(tile_w, -1);
                tile.set_tooltip_text(Some(&wall.name));

                let pic = gtk::Picture::new();
                pic.set_content_fit(gtk::ContentFit::Cover);
                pic.set_can_shrink(true);
                pic.set_overflow(gtk::Overflow::Hidden);
                pic.set_size_request(pic_w, pic_h);
                if let Some(t) = thumbs.get(&wall.path) {
                    pic.set_paintable(Some(t));
                }
                tile.append(&pic);
                let name = gtk::Label::new(Some(&wall.name));
                name.add_css_class("name");
                name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                name.set_max_width_chars((pic_w / 8).max(6));
                tile.append(&name);

                let click = gtk::GestureClick::new();
                let cb = on_click.clone();
                click.connect_pressed(move |_, _, _, _| cb(idx));
                tile.add_controller(click);

                grid.attach(&tile, (i % cols) as i32, (i / cols) as i32, 1, 1);
                pictures.insert(wall.path.clone(), pic);
                tiles.push(tile);
            }
            self.content.append(&grid);
        }
        *self.tiles.borrow_mut() = tiles;
        *self.pictures.borrow_mut() = pictures;
        let (_, nat, _, _) = self.content.measure(gtk::Orientation::Vertical, -1);
        self.scroll
            .set_min_content_height(nat.min(self.scroll.max_content_height()));
        self.select(selected);
    }

    pub fn select(&self, selected: usize) {
        for (i, t) in self.tiles.borrow().iter().enumerate() {
            if i == selected {
                t.add_css_class("selected");
                ensure_visible(&self.scroll, self.content.upcast_ref(), t.upcast_ref());
            } else {
                t.remove_css_class("selected");
            }
        }
    }

    pub fn set_thumb(&self, path: &PathBuf, tex: &gdk::Texture) {
        if let Some(p) = self.pictures.borrow().get(path) {
            p.set_paintable(Some(tex));
        }
    }
}
