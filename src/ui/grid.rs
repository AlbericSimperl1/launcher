//! App-grid met pin-sectie, divider en contextmenu-hooks.

use crate::desktop::DesktopApp;
use crate::ui::results::ensure_visible;
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// Berekent de nieuwe selectie in een platte lijst die uit twee secties bestaat
/// (`pinned` items, daarna de rest), elk met `cols` kolommen.
pub fn navigate(i: usize, n: usize, pinned: usize, cols: usize, dir: Dir) -> usize {
    if n == 0 || cols == 0 {
        return 0;
    }
    let i = i.min(n - 1);
    let (start, end) = if i < pinned { (0, pinned) } else { (pinned, n) };
    let (row, col) = ((i - start) / cols, (i - start) % cols);
    match dir {
        Dir::Left => i.saturating_sub(1),
        Dir::Right => (i + 1).min(n - 1),
        Dir::Down => {
            if i + cols < end {
                i + cols
            } else if (end - 1 - start) / cols > row {
                end - 1
            } else if i < pinned && pinned < n {
                (pinned + col).min(n - 1)
            } else {
                i
            }
        }
        Dir::Up => {
            if row > 0 {
                i - cols
            } else if i >= pinned && pinned > 0 {
                (((pinned - 1) / cols) * cols + col).min(pinned - 1)
            } else {
                i
            }
        }
    }
}

pub struct Grid {
    pub scroll: gtk::ScrolledWindow,
    content: gtk::Box,
    pinned: gtk::Grid,
    divider: gtk::Box,
    rest: gtk::Grid,
    tiles: RefCell<Vec<gtk::Box>>,
}

fn app_icon(icon: &str, size: i32) -> gtk::Image {
    let img = if icon.starts_with('/') {
        gtk::Image::from_file(icon)
    } else {
        let known = gtk::gdk::Display::default()
            .map(|d| gtk::IconTheme::for_display(&d).has_icon(icon))
            .unwrap_or(false);
        gtk::Image::from_icon_name(if known {
            icon
        } else {
            "application-x-executable"
        })
    };
    img.set_pixel_size(size);
    img
}

impl Grid {
    pub fn new(max_height: i32) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let make_grid = || {
            let g = gtk::Grid::new();
            g.set_column_homogeneous(true);
            g.set_row_spacing(2);
            g.set_column_spacing(2);
            g.set_halign(gtk::Align::Start);
            g
        };
        let pinned = make_grid();
        let rest = make_grid();
        let divider = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        divider.add_css_class("divider");
        content.append(&pinned);
        content.append(&divider);
        content.append(&rest);

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
            pinned,
            divider,
            rest,
            tiles: RefCell::new(Vec::new()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn populate(
        &self,
        pinned: &[DesktopApp],
        rest: &[DesktopApp],
        cols: usize,
        icon_size: i32,
        tile_w: i32,
        selected: usize,
        on_click: Rc<dyn Fn(usize)>,
        on_context: Rc<dyn Fn(usize, &gtk::Widget, f64, f64)>,
    ) {
        for g in [&self.pinned, &self.rest] {
            while let Some(c) = g.first_child() {
                g.remove(&c);
            }
        }
        let mut tiles = Vec::with_capacity(pinned.len() + rest.len());
        for (section, apps) in [(&self.pinned, pinned), (&self.rest, rest)] {
            for (i, app) in apps.iter().enumerate() {
                let idx = tiles.len();
                let tile = gtk::Box::new(gtk::Orientation::Vertical, 6);
                tile.add_css_class("tile");
                tile.set_size_request(tile_w, -1);
                tile.set_tooltip_text(Some(&app.name));
                tile.append(&app_icon(&app.icon, icon_size));
                let label = gtk::Label::new(Some(&app.name));
                label.add_css_class("name");
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                label.set_max_width_chars(((tile_w / 8).max(8)) as i32);
                label.set_justify(gtk::Justification::Center);
                tile.append(&label);

                let click = gtk::GestureClick::builder().button(0).build();
                let (cb, ctx, widget) = (on_click.clone(), on_context.clone(), tile.clone());
                click.connect_pressed(move |g, _, x, y| match g.current_button() {
                    1 => cb(idx),
                    3 => ctx(idx, widget.upcast_ref(), x, y),
                    _ => {}
                });
                tile.add_controller(click);

                section.attach(&tile, (i % cols) as i32, (i / cols) as i32, 1, 1);
                tiles.push(tile);
            }
        }
        self.pinned.set_visible(!pinned.is_empty());
        self.divider
            .set_visible(!pinned.is_empty() && !rest.is_empty());
        self.rest.set_visible(!rest.is_empty());
        *self.tiles.borrow_mut() = tiles;
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
}
