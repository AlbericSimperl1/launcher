//! Resultaatgebied onder de zoekbalk: lijst, app-grid, calculator en lege staat.

use crate::modes::ListRow;
use crate::ui::{grid::Grid, icons};
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::RefCell;
use std::rc::Rc;

pub enum CalcView {
    Idle(String),
    Value { expr: String, result: String },
    Error(String),
}

pub struct Results {
    pub revealer: gtk::Revealer,
    pub grid: Grid,
    stack: gtk::Stack,
    list_scroll: gtk::ScrolledWindow,
    list_box: gtk::Box,
    rows: RefCell<Vec<gtk::Box>>,
    empty: gtk::Label,
    calc_expr: gtk::Label,
    calc_result: gtk::Label,
    calc_hint: gtk::Label,
}

pub fn ensure_visible(scroll: &gtk::ScrolledWindow, container: &gtk::Widget, child: &gtk::Widget) {
    let (scroll, container, child) = (scroll.clone(), container.clone(), child.clone());
    glib::idle_add_local_once(move || {
        let Some(b) = child.compute_bounds(&container) else {
            return;
        };
        let adj = scroll.vadjustment();
        let (y, h) = (b.y() as f64, b.height() as f64);
        if y < adj.value() {
            adj.set_value((y - 6.0).max(0.0));
        } else if y + h > adj.value() + adj.page_size() {
            adj.set_value(y + h - adj.page_size() + 6.0);
        }
    });
}

impl Results {
    pub fn new(max_height: i32, anim_ms: u32) -> Self {
        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(anim_ms);
        stack.set_hhomogeneous(true);
        stack.set_vhomogeneous(false);

        let list_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let list_scroll = gtk::ScrolledWindow::new();
        list_scroll.set_child(Some(&list_box));
        list_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        list_scroll.set_propagate_natural_height(true);
        list_scroll.set_max_content_height(max_height);
        list_scroll.set_focusable(false);
        list_scroll.set_overlay_scrolling(true);

        stack.add_named(&list_scroll, Some("list"));

        let grid = Grid::new(max_height);
        stack.add_named(&grid.scroll, Some("grid"));

        let calc = gtk::Box::new(gtk::Orientation::Vertical, 4);
        calc.set_margin_top(18);
        calc.set_margin_bottom(14);
        calc.set_margin_start(14);
        calc.set_margin_end(14);
        let calc_expr = gtk::Label::new(None);
        calc_expr.add_css_class("calc-expr");
        calc_expr.set_xalign(0.0);
        calc_expr.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let calc_result = gtk::Label::new(None);
        calc_result.set_xalign(0.0);
        calc_result.set_selectable(false);
        let calc_hint = gtk::Label::new(None);
        calc_hint.add_css_class("calc-hint");
        calc_hint.set_xalign(0.0);
        calc.append(&calc_expr);
        calc.append(&calc_result);
        calc.append(&calc_hint);
        stack.add_named(&calc, Some("calc"));

        let empty = gtk::Label::new(None);
        empty.add_css_class("empty");
        stack.add_named(&empty, Some("empty"));

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let sep = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        sep.add_css_class("results-sep");
        content.append(&sep);
        content.append(&stack);

        let revealer = gtk::Revealer::new();
        revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        revealer.set_transition_duration(anim_ms + 40);
        revealer.set_child(Some(&content));
        revealer.set_reveal_child(false);

        Self {
            revealer,
            grid,
            stack,
            list_scroll,
            list_box,
            rows: RefCell::new(Vec::new()),
            empty,
            calc_expr,
            calc_result,
            calc_hint,
        }
    }

    pub fn set_open(&self, open: bool) {
        if self.revealer.reveals_child() != open {
            self.revealer.set_reveal_child(open);
        }
    }

    pub fn show_grid(&self) {
        self.stack.set_visible_child_name("grid");
    }

    pub fn show_empty(&self, text: &str) {
        self.empty.set_text(text);
        self.stack.set_visible_child_name("empty");
    }

    pub fn show_calc(&self, view: CalcView) {
        self.calc_result.remove_css_class("calc-error");
        self.calc_result.remove_css_class("calc-result");
        match view {
            CalcView::Idle(hint) => {
                self.calc_expr.set_text("");
                self.calc_result.add_css_class("calc-result");
                self.calc_result.set_text("0");
                self.calc_hint.set_text(&hint);
            }
            CalcView::Value { expr, result } => {
                self.calc_expr.set_text(&expr);
                self.calc_result.add_css_class("calc-result");
                self.calc_result.set_text(&format!("= {result}"));
                self.calc_hint.set_text(
                    "Enter: kopieer · Ctrl+Enter / Tab: verder rekenen met dit resultaat",
                );
            }
            CalcView::Error(msg) => {
                self.calc_expr.set_text("");
                self.calc_result.add_css_class("calc-error");
                self.calc_result.set_text(&msg);
                self.calc_hint.set_text("");
            }
        }
        self.stack.set_visible_child_name("calc");
    }

    pub fn set_rows(&self, rows: &[ListRow], selected: usize, on_click: Rc<dyn Fn(usize)>) {
        while let Some(c) = self.list_box.first_child() {
            self.list_box.remove(&c);
        }
        let mut widgets = Vec::with_capacity(rows.len());
        for (i, row) in rows.iter().enumerate() {
            let r = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            r.add_css_class("row");
            r.append(&icons::image(row.icon, "fg", 20));

            let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
            text.set_hexpand(true);
            text.set_valign(gtk::Align::Center);
            let title = gtk::Label::new(Some(&row.title));
            title.add_css_class("title");
            title.set_xalign(0.0);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            text.append(&title);
            if !row.subtitle.is_empty() {
                let sub = gtk::Label::new(Some(&row.subtitle));
                sub.add_css_class("subtitle");
                sub.set_xalign(0.0);
                sub.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                text.append(&sub);
            }
            r.append(&text);

            let click = gtk::GestureClick::new();
            let cb = on_click.clone();
            click.connect_pressed(move |_, _, _, _| cb(i));
            r.add_controller(click);

            self.list_box.append(&r);
            widgets.push(r);
        }
        let (_, nat, _, _) = self.list_box.measure(gtk::Orientation::Vertical, -1);
        self.list_scroll
            .set_min_content_height(nat.min(self.list_scroll.max_content_height()));
        *self.rows.borrow_mut() = widgets;
        self.stack.set_visible_child_name("list");
        self.select_row(selected);
    }

    pub fn select_row(&self, selected: usize) {
        for (i, r) in self.rows.borrow().iter().enumerate() {
            if i == selected {
                r.add_css_class("selected");
                ensure_visible(
                    &self.list_scroll,
                    self.list_box.upcast_ref(),
                    r.upcast_ref(),
                );
            } else {
                r.remove_css_class("selected");
            }
        }
    }
}
