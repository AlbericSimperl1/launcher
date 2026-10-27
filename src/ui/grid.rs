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

/// Berekent de nieuwe selectie in een platte lijst die uit opeenvolgende secties bestaat
/// (`sections` = lengte per sectie, bv. pinned + rest), elk met `cols` kolommen.
pub fn navigate(i: usize, sections: &[usize], cols: usize, dir: Dir) -> usize {
    let mut bounds = Vec::new();
    let mut start = 0;
    for &len in sections.iter().filter(|&&l| l > 0) {
        bounds.push((start, start + len));
        start += len;
    }
    let n = start;
    if n == 0 || cols == 0 {
        return 0;
    }
    let i = i.min(n - 1);
    let k = bounds.iter().position(|&(_, e)| i < e).unwrap_or(0);
    let (start, end) = bounds[k];
    let (row, col) = ((i - start) / cols, (i - start) % cols);
    match dir {
        Dir::Left => i.saturating_sub(1),
        Dir::Right => (i + 1).min(n - 1),
        Dir::Down => {
            if i + cols < end {
                i + cols
            } else if (end - 1 - start) / cols > row {
                end - 1
            } else if let Some(&(s, e)) = bounds.get(k + 1) {
                (s + col).min(e - 1)
            } else {
                i
            }
        }
        Dir::Up => {
            if row > 0 {
                i - cols
            } else if k > 0 {
                let (s, e) = bounds[k - 1];
                (s + ((e - 1 - s) / cols) * cols + col).min(e - 1)
            } else {
                i
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections() {
        // secties van 2, 0 (leeg), 5 en 1 item(s), 3 kolommen
        let s = [2, 0, 5, 1];
        assert_eq!(navigate(0, &s, 3, Dir::Down), 2);
        assert_eq!(navigate(1, &s, 3, Dir::Down), 3);
        assert_eq!(navigate(3, &s, 3, Dir::Down), 6);
        assert_eq!(navigate(6, &s, 3, Dir::Down), 7);
        assert_eq!(navigate(7, &s, 3, Dir::Up), 5);
        assert_eq!(navigate(4, &s, 3, Dir::Up), 1);
        assert_eq!(navigate(2, &s, 3, Dir::Up), 0);
        assert_eq!(navigate(7, &s, 3, Dir::Down), 7);
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

fn grid_tile(app: &DesktopApp, icon_size: i32, tile_w: i32, show_name: bool) -> gtk::Box {
    let tile = gtk::Box::new(gtk::Orientation::Vertical, 6);
    tile.add_css_class("tile");
    tile.append(&app_icon(&app.icon, icon_size));
    if !show_name {
        return tile;
    }
    let label = gtk::Label::new(Some(&app.name));
    label.add_css_class("name");
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_max_width_chars(((tile_w / 8).max(8)) as i32);
    label.set_justify(gtk::Justification::Center);
    tile.append(&label);
    tile
}

fn list_tile(app: &DesktopApp, icon_size: i32) -> gtk::Box {
    let tile = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    tile.add_css_class("tile");
    tile.add_css_class("list");
    tile.append(&app_icon(&app.icon, icon_size));
    let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    for (s, class) in [(&app.name, "name"), (&app.exec, "exec")] {
        let l = gtk::Label::new(Some(s));
        l.add_css_class(class);
        l.set_xalign(0.0);
        l.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&l);
    }
    tile.append(&text);
    tile
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
        list: bool,
        show_names: bool,
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
                let tile = if list {
                    list_tile(app, icon_size)
                } else {
                    grid_tile(app, icon_size, tile_w, show_names)
                };
                tile.set_size_request(tile_w, -1);
                tile.set_tooltip_text(Some(&app.name));

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
