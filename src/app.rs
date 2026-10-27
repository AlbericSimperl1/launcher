//! Root-state van de launcher: venster, event-afhandeling en koppeling tussen UI en modi.

use crate::bookmarks::{self, Bookmark};
use crate::config::{self, Config};
use crate::desktop::{self, DesktopApp};
use crate::modes::files::{self, FileEntry};
use crate::modes::wallpaper::{self, Section, Wall};
use crate::modes::{apps, browser, calc, ListRow, Mode, RowAction};
use crate::state::State;
use crate::ui::bar::{self, Bar};
use crate::ui::grid::{self, Dir};
use crate::ui::results::{CalcView, Results};
use crate::ui::{icons, theme};
use crate::util::{self, Fuzzy};

use gtk::gdk::{self, Key, ModifierType};
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

const MAX_BOOKMARK_ROWS: usize = 1000;
/// Synthetische app in de apps-lijst die het settingspanel opent.
const SETTINGS_ID: &str = "rust-launcher-settings";

/// Berichten van achtergrondtaken (tokio) naar de GTK-mainloop.
pub enum Msg {
    Apps(Vec<DesktopApp>),
    Bookmarks(Vec<Bookmark>),
    FilesIndexed {
        root: PathBuf,
        hidden: bool,
        entries: Arc<Vec<FileEntry>>,
    },
    FileResults {
        gen: u64,
        entries: Vec<FileEntry>,
    },
    Walls(Vec<Section>),
    Thumb {
        path: PathBuf,
        tex: gdk::Texture,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum How {
    Primary,
    Manager,
    Editor,
}

struct Ui {
    visible: bool,
    mode: Option<Mode>,
    explicit: bool,
    selected: usize,

    apps: Vec<DesktopApp>,
    view_apps: Vec<DesktopApp>,
    pinned_count: usize,
    state: State,

    rows: Vec<ListRow>,

    files_root: PathBuf,
    files_cwd: PathBuf,
    show_hidden: bool,
    index_cache: HashMap<(PathBuf, bool), Arc<Vec<FileEntry>>>,
    indexing: HashSet<(PathBuf, bool)>,
    file_gen: u64,

    bookmarks: Vec<Bookmark>,
    bookmarks_at: Option<Instant>,

    calc_value: Option<f64>,
    calc_text: Option<String>,

    walls: Option<Vec<Section>>,
    /// Gefilterde wallpapers in weergavevolgorde, plus de lengte per thema-sectie.
    view_walls: Vec<Wall>,
    wall_sections: Vec<usize>,
    thumbs: HashMap<PathBuf, gdk::Texture>,
    thumbs_loading: HashSet<PathBuf>,

    /// Kaart staat links op volle hoogte; idle-modus is dan Apps (volledige lijst).
    sidebar: bool,
    /// Geopend door de muis tegen de linkerrand: sluit weer als de muis de kaart verlaat.
    by_hover: bool,
}

pub struct Launcher {
    window: gtk::ApplicationWindow,
    card: gtk::Box,
    bar: Bar,
    results: Results,
    cfg: RefCell<Rc<Config>>,
    rt: Handle,
    tx: UnboundedSender<Msg>,
    suppress: Cell<bool>,
    ui: RefCell<Ui>,
    css: RefCell<Option<gtk::CssProvider>>,
    watcher: RefCell<Option<gtk::gio::FileMonitor>>,
    reload_pending: Cell<bool>,
    /// Hyprlands eigen (size, passes), bewaard zolang wij de blur-sterkte overschrijven.
    saved_blur: Cell<Option<(i64, i64)>>,
    /// Onzichtbare strook tegen de linkerrand (placement = "hover").
    edge: RefCell<Option<gtk::ApplicationWindow>>,
    leave_timer: RefCell<Option<glib::SourceId>>,
    menu_open: Cell<bool>,
}

impl Launcher {
    pub fn new(
        app: &gtk::Application,
        cfg: Config,
        rt: Handle,
        tx: UnboundedSender<Msg>,
    ) -> Rc<Self> {
        let dark = theme::resolve_dark(&cfg.appearance);
        let blur = theme::resolve_blur(&cfg.appearance);
        tracing::info!("thema: dark={dark} blur={blur}");
        icons::install(dark);

        let provider = gtk::CssProvider::new();
        provider.load_from_data(&theme::css(&cfg.appearance, dark, blur));
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
        }

        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .decorated(false)
            .build();
        window.add_css_class("launcher");
        window.init_layer_shell();
        window.set_namespace("rust-launcher");
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);
        window.set_exclusive_zone(-1);
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            window.set_anchor(edge, true);
        }

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_hexpand(true);
        root.set_vexpand(true);

        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("card");
        card.set_width_request(cfg.appearance.width);
        card.set_halign(gtk::Align::Center);
        card.set_valign(gtk::Align::Center);
        card.set_vexpand(true);
        root.append(&card);
        window.set_child(Some(&root));

        let files_root = {
            let p = util::expand_tilde(&cfg.files.start_dir);
            if p.is_dir() {
                p
            } else {
                util::home()
            }
        };

        let launcher = Rc::new_cyclic(|weak: &std::rc::Weak<Launcher>| {
            let w = weak.clone();
            let bar = Bar::new(Rc::new(move |m| {
                if let Some(l) = w.upgrade() {
                    l.choose_mode(m);
                }
            }));
            let results = Results::new(cfg.appearance.max_height, 0);
            card.append(&bar.root);
            card.append(&results.revealer);
            Launcher {
                window,
                card,
                bar,
                results,
                rt,
                tx,
                suppress: Cell::new(false),
                ui: RefCell::new(Ui {
                    visible: false,
                    mode: None,
                    explicit: false,
                    selected: 0,
                    apps: Vec::new(),
                    view_apps: Vec::new(),
                    pinned_count: 0,
                    state: State::load(),
                    rows: Vec::new(),
                    files_cwd: files_root.clone(),
                    files_root,
                    show_hidden: cfg.files.show_hidden,
                    index_cache: HashMap::new(),
                    indexing: HashSet::new(),
                    file_gen: 0,
                    bookmarks: Vec::new(),
                    bookmarks_at: None,
                    calc_value: None,
                    calc_text: None,
                    walls: None,
                    view_walls: Vec::new(),
                    wall_sections: Vec::new(),
                    thumbs: HashMap::new(),
                    thumbs_loading: HashSet::new(),
                    sidebar: false,
                    by_hover: false,
                }),
                cfg: RefCell::new(Rc::new(cfg)),
                css: RefCell::new(Some(provider)),
                watcher: RefCell::new(None),
                reload_pending: Cell::new(false),
                saved_blur: Cell::new(None),
                edge: RefCell::new(None),
                leave_timer: RefCell::new(None),
                menu_open: Cell::new(false),
            }
        });
        launcher.connect_signals(root);
        launcher.watch_config();
        launcher.update_edge();
        launcher.reload_apps();
        launcher
    }

    /// Hyprland kent geen blur-sterkte per layer: zolang de launcher open is zetten we de
    /// globale `decoration:blur` op onze waarden, en bij sluiten weer terug.
    fn apply_blur_strength(&self) {
        let cfg = self.cfg();
        let a = &cfg.appearance;
        if !theme::resolve_blur(a) || (a.blur_radius == 0 && a.blur_passes == 0) {
            self.restore_blur_strength();
            return;
        }
        if self.saved_blur.get().is_none() {
            self.saved_blur.set(util::hypr_blur_get());
        }
        let Some((size, passes)) = self.saved_blur.get() else {
            return;
        };
        let pick = |v: u32, orig: i64| if v > 0 { v as i64 } else { orig };
        util::hypr_blur_set(pick(a.blur_radius, size), pick(a.blur_passes, passes));
    }

    fn restore_blur_strength(&self) {
        if let Some((size, passes)) = self.saved_blur.take() {
            util::hypr_blur_set(size, passes);
        }
    }

    fn cfg(&self) -> Rc<Config> {
        self.cfg.borrow().clone()
    }

    fn connect_signals(self: &Rc<Self>, root: gtk::Box) {
        let me = self.clone();
        self.bar
            .entry
            .connect_changed(move |_| me.on_text_changed());

        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        let me = self.clone();
        key.connect_key_pressed(move |_, keyval, _, state| me.on_key(keyval, state));
        self.window.add_controller(key);

        // Klik buiten de kaart sluit de launcher.
        let click = gtk::GestureClick::new();
        let me = self.clone();
        let root2 = root.clone();
        click.connect_pressed(move |_, _, x, y| {
            if let Some(b) = me.card.compute_bounds(&root2) {
                if !b.contains_point(&gtk::graphene::Point::new(x as f32, y as f32)) {
                    me.hide();
                }
            }
        });
        root.add_controller(click);

        // Via hover geopend: sluiten zodra de muis de kaart verlaat (met korte vertraging).
        let motion = gtk::EventControllerMotion::new();
        let me = self.clone();
        motion.connect_enter(move |_, _, _| me.cancel_leave());
        let me = self.clone();
        motion.connect_leave(move |_| {
            if !me.ui.borrow().by_hover || me.menu_open.get() {
                return;
            }
            me.cancel_leave();
            let me2 = me.clone();
            let id = glib::timeout_add_local_once(Duration::from_millis(250), move || {
                me2.leave_timer.borrow_mut().take();
                me2.hide();
            });
            *me.leave_timer.borrow_mut() = Some(id);
        });
        self.card.add_controller(motion);
    }

    fn cancel_leave(&self) {
        if let Some(id) = self.leave_timer.borrow_mut().take() {
            id.remove();
        }
    }

    /// Maakt of verwijdert de hover-strook tegen de linkerrand, afhankelijk van `placement`.
    fn update_edge(self: &Rc<Self>) {
        let want = self.cfg().appearance.placement == "hover";
        let mut edge = self.edge.borrow_mut();
        if !want {
            if let Some(w) = edge.take() {
                w.destroy();
            }
            return;
        }
        if edge.is_some() {
            return;
        }
        let w = gtk::ApplicationWindow::builder().decorated(false).build();
        if let Some(app) = self.window.application() {
            w.set_application(Some(&app));
        }
        w.add_css_class("edge");
        w.init_layer_shell();
        w.set_namespace("rust-launcher-edge");
        w.set_layer(Layer::Top);
        w.set_keyboard_mode(KeyboardMode::None);
        w.set_exclusive_zone(0);
        for e in [Edge::Left, Edge::Top, Edge::Bottom] {
            w.set_anchor(e, true);
        }
        w.set_size_request(2, -1);
        w.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
        let motion = gtk::EventControllerMotion::new();
        let me = self.clone();
        motion.connect_enter(move |_, _, _| me.show_as(true));
        w.add_controller(motion);
        w.present();
        *edge = Some(w);
    }

    /// Positie en grootte van de kaart: center (`width`, begrensde hoogte) of sidebar
    /// (`sidebar_width` × schermhoogte, links).
    fn apply_placement(&self) {
        let cfg = self.cfg();
        let a = &cfg.appearance;
        let sidebar = self.ui.borrow().sidebar;
        let (width, align, valign, margin) = if sidebar {
            (a.sidebar_width, gtk::Align::Start, gtk::Align::Fill, 8)
        } else {
            (a.width, gtk::Align::Center, gtk::Align::Center, 0)
        };
        self.card.set_width_request(width);
        self.card.set_halign(align);
        self.card.set_valign(valign);
        self.card.set_margin_top(margin);
        self.card.set_margin_bottom(margin);
        self.card.set_margin_start(margin);
        self.results.set_fill(sidebar, a.max_height);
    }

    pub fn spawn_receiver(self: &Rc<Self>, mut rx: UnboundedReceiver<Msg>) {
        let me = self.clone();
        glib::spawn_future_local(async move {
            while let Some(msg) = rx.recv().await {
                me.handle(msg);
            }
        });
    }

    // ---------------------------------------------------------------- zichtbaarheid

    pub fn toggle(self: &Rc<Self>) {
        let visible = self.ui.borrow().visible;
        if visible {
            self.hide();
        } else {
            self.show();
        }
    }

    pub fn show(self: &Rc<Self>) {
        self.show_as(false);
    }

    /// `hover`: geopend via de linkerrand, altijd als sidebar.
    fn show_as(self: &Rc<Self>, hover: bool) {
        {
            let mut ui = self.ui.borrow_mut();
            if ui.visible {
                drop(ui);
                self.window.present();
                self.bar.entry.grab_focus();
                return;
            }
            ui.visible = true;
        }
        *self.cfg.borrow_mut() = Rc::new(config::load());
        let sidebar = hover || self.cfg().appearance.placement == "sidebar";
        {
            let mut ui = self.ui.borrow_mut();
            ui.show_hidden = self.cfg().files.show_hidden;
            ui.sidebar = sidebar;
            ui.by_hover = hover;
            ui.mode = sidebar.then_some(Mode::Apps);
        }
        self.apply_placement();
        self.bar.set_mode(self.ui.borrow().mode);
        self.refresh(true);
        self.apply_blur_strength();
        self.window.set_visible(true);
        self.window.present();
        self.bar.entry.grab_focus();
        self.reload_apps();
    }

    pub fn hide(self: &Rc<Self>) {
        {
            let mut ui = self.ui.borrow_mut();
            if !ui.visible {
                return;
            }
            ui.visible = false;
            ui.by_hover = false;
        }
        self.cancel_leave();
        self.window.set_visible(false);
        self.restore_blur_strength();
        self.reset();
        self.reload_apps();
    }

    pub fn open_settings(self: &Rc<Self>) {
        self.show();
        self.clear_entry();
        self.choose_mode(Mode::Settings);
    }

    pub fn unhide_all(self: &Rc<Self>) {
        self.ui.borrow_mut().state.unhide_all();
    }

    fn reset(self: &Rc<Self>) {
        {
            let mut ui = self.ui.borrow_mut();
            ui.mode = ui.sidebar.then_some(Mode::Apps);
            ui.explicit = false;
            ui.selected = 0;
            ui.rows.clear();
            ui.view_apps.clear();
            ui.files_cwd = ui.files_root.clone();
            ui.show_hidden = self.cfg().files.show_hidden;
            ui.index_cache.clear();
            ui.indexing.clear();
            ui.file_gen += 1;
            ui.calc_value = None;
            ui.calc_text = None;
        }
        self.suppress.set(true);
        self.bar.entry.set_text("");
        self.suppress.set(false);
        self.bar.set_mode(None);
        self.results.set_open(false);
    }

    fn back_to_general(self: &Rc<Self>) {
        let mode = {
            let mut ui = self.ui.borrow_mut();
            ui.mode = ui.sidebar.then_some(Mode::Apps);
            ui.explicit = false;
            ui.selected = 0;
            ui.rows.clear();
            ui.files_cwd = ui.files_root.clone();
            ui.file_gen += 1;
            ui.calc_value = None;
            ui.calc_text = None;
            ui.mode
        };
        self.clear_entry();
        self.bar.set_mode(mode);
        self.bar.set_crumb(None);
        self.refresh(true);
        self.bar.entry.grab_focus_without_selecting();
    }

    // ---------------------------------------------------------------- config-watch & reload

    fn watch_config(self: &Rc<Self>) {
        let dir = util::config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let gfile = gtk::gio::File::for_path(&dir);
        match gfile.monitor_directory(
            gtk::gio::FileMonitorFlags::WATCH_MOVES,
            gtk::gio::Cancellable::NONE,
        ) {
            Ok(m) => {
                let me = self.clone();
                // Atomisch opslaan (tmp-bestand + rename) meldt config.toml als `other`.
                m.connect_changed(move |_, file, other, _| {
                    let is_cfg = |f: &gtk::gio::File| {
                        f.basename().as_deref() == Some(Path::new("config.toml"))
                    };
                    if is_cfg(file) || other.is_some_and(is_cfg) {
                        me.schedule_reload();
                    }
                });
                *self.watcher.borrow_mut() = Some(m);
                tracing::info!("config-watch actief op {}", dir.display());
            }
            Err(e) => tracing::warn!("config-watch mislukt: {e}"),
        }
    }

    fn schedule_reload(self: &Rc<Self>) {
        // Editors triggeren vaak meerdere events (write + rename).
        // Debounce: alleen de eerste keer een timeout inplannen.
        if self.reload_pending.replace(true) {
            return;
        }
        let me = self.clone();
        glib::timeout_add_local_once(Duration::from_millis(400), move || {
            me.reload_pending.set(false);
            me.reload();
        });
    }

    pub fn reload(self: &Rc<Self>) {
        tracing::info!("config herladen");
        *self.cfg.borrow_mut() = Rc::new(config::load());
        let cfg = self.cfg();

        let dark = theme::resolve_dark(&cfg.appearance);
        let blur = theme::resolve_blur(&cfg.appearance);
        icons::install(dark);

        if let Some(display) = gtk::gdk::Display::default() {
            let old = self.css.borrow_mut().take();
            if let Some(old_provider) = old {
                gtk::style_context_remove_provider_for_display(&display, &old_provider);
            }
            let provider = gtk::CssProvider::new();
            provider.load_from_data(&theme::css(&cfg.appearance, dark, blur));
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
            *self.css.borrow_mut() = Some(provider);
        }

        self.apply_placement();
        self.update_edge();
        if self.ui.borrow().visible {
            self.apply_blur_strength();
        }

        {
            let mut ui = self.ui.borrow_mut();
            let new_root = util::expand_tilde(&cfg.files.start_dir);
            if new_root.is_dir() {
                ui.files_root = new_root;
            }
            ui.files_cwd = ui.files_root.clone();
            ui.show_hidden = cfg.files.show_hidden;
            ui.index_cache.clear();
            ui.indexing.clear();
            ui.file_gen += 1;
            ui.selected = 0;
        }

        self.refresh(true);
        self.reload_apps();
    }

    // ---------------------------------------------------------------- modus & invoer

    fn choose_mode(self: &Rc<Self>, mode: Mode) {
        {
            let mut ui = self.ui.borrow_mut();
            ui.mode = Some(mode);
            ui.explicit = true;
            ui.selected = 0;
        }
        self.bar.set_mode(Some(mode));
        if mode == Mode::Browser {
            self.ensure_bookmarks();
        }
        if mode == Mode::Wallpaper {
            self.scan_walls();
        }
        if mode == Mode::Settings {
            self.results.settings.build(&self.cfg());
            self.refresh(true);
            self.results.settings.focus_first();
            return;
        }
        self.bar.entry.grab_focus_without_selecting();
        self.refresh(true);
    }

    fn on_text_changed(self: &Rc<Self>) {
        if self.suppress.get() {
            return;
        }
        let text = self.bar.entry.text().to_string();
        let explicit = self.ui.borrow().explicit;
        if !explicit {
            if let Some((mode, rest)) = bar::detect_flag(&text) {
                self.suppress.set(true);
                self.bar.entry.set_text(&rest);
                self.bar.entry.set_position(0);
                self.suppress.set(false);
                self.choose_mode(mode);
                return;
            }
        }
        let new_mode = {
            let mut ui = self.ui.borrow_mut();
            let before = ui.mode;
            if ui.mode.is_none() && !text.is_empty() {
                ui.mode = Some(Mode::Apps);
            } else if ui.mode == Some(Mode::Apps) && text.is_empty() && !ui.explicit && !ui.sidebar
            {
                ui.mode = None;
            }
            (before != ui.mode).then_some(ui.mode)
        };
        if let Some(m) = new_mode {
            self.bar.set_mode(m);
        }
        self.refresh(true);
    }

    fn refresh(self: &Rc<Self>, reset_selection: bool) {
        let text = self.bar.entry.text().to_string();
        let mode = {
            let mut ui = self.ui.borrow_mut();
            if reset_selection {
                ui.selected = 0;
            }
            ui.mode
        };
        match mode {
            None => {
                self.bar.set_crumb(None);
                self.results.set_open(false);
            }
            Some(Mode::Apps) => self.refresh_apps(&text),
            Some(Mode::Files) => self.refresh_files(&text),
            Some(Mode::Calc) => self.refresh_calc(&text),
            Some(Mode::Browser) => self.refresh_browser(&text),
            Some(Mode::Wallpaper) => self.refresh_walls(&text),
            // Niet herbouwen: reload() na elke wijziging zou anders de focus wegnemen.
            Some(Mode::Settings) => {
                self.bar.set_crumb(None);
                self.results.show_settings();
                self.results.set_open(true);
            }
        }
    }

    // ---------------------------------------------------------------- apps

    fn reload_apps(&self) {
        let (tx, extra) = (
            self.tx.clone(),
            self.cfg()
                .apps
                .extra_dirs
                .iter()
                .map(|d| util::expand_tilde(d))
                .collect::<Vec<_>>(),
        );
        self.rt.spawn_blocking(move || {
            let mut apps = desktop::load_apps(&extra);
            apps.push(DesktopApp {
                id: SETTINGS_ID.into(),
                name: "Launcher Settings".into(),
                icon: "preferences-system".into(),
                keywords: "settings instellingen config launcher".into(),
                exec: "rust-launcher --settings".into(),
                path: PathBuf::new(),
            });
            apps.sort_by_key(|a| a.name.to_lowercase());
            let _ = tx.send(Msg::Apps(apps));
        });
    }

    fn refresh_apps(self: &Rc<Self>, text: &str) {
        let (pinned, rest, selected) = {
            let mut ui = self.ui.borrow_mut();
            let (p, r) = apps::build_view(&ui.apps, &ui.state, text);
            ui.pinned_count = p.len();
            ui.view_apps = p.iter().chain(r.iter()).cloned().collect();
            ui.selected = ui.selected.min(ui.view_apps.len().saturating_sub(1));
            (p, r, ui.selected)
        };
        if pinned.is_empty() && rest.is_empty() {
            self.results.show_empty("Geen apps gevonden");
        } else {
            let cols = self.app_columns();
            let tile_w = (self.card.width_request() - 28 - 24) / cols as i32;
            let (a, b) = (self.clone(), self.clone());
            self.results.grid.populate(
                &pinned,
                &rest,
                cols,
                self.cfg().apps.icon_size,
                tile_w,
                selected,
                Rc::new(move |i| a.activate_index(i)),
                Rc::new(move |i, w, x, y| b.show_app_menu(i, w, x, y)),
                self.cfg().apps.layout == "list",
            );
            self.results.show_grid();
        }
        self.results.set_open(true);
    }

    fn app_columns(&self) -> usize {
        if self.cfg().apps.layout == "list" {
            1
        } else {
            self.cfg().apps.columns.max(1)
        }
    }

    fn show_app_menu(self: &Rc<Self>, idx: usize, widget: &gtk::Widget, x: f64, y: f64) {
        let (app, pinned) = {
            let mut ui = self.ui.borrow_mut();
            let Some(app) = ui.view_apps.get(idx).cloned() else {
                return;
            };
            ui.selected = idx;
            let pinned = ui.state.is_pinned(&app.id);
            (app, pinned)
        };
        self.results.grid.select(idx);

        let pop = gtk::Popover::new();
        pop.add_css_class("ctx-menu");
        pop.set_has_arrow(false);
        pop.set_autohide(true);
        pop.set_parent(widget);
        pop.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        pop.set_position(gtk::PositionType::Bottom);

        let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let item = |label: &str| {
            let b = gtk::Button::with_label(label);
            if let Some(l) = b.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
                l.set_xalign(0.0);
            }
            b
        };

        let pin_btn = item(if pinned { "Losmaken" } else { "Pin naar boven" });
        let (me, p, id) = (self.clone(), pop.clone(), app.id.clone());
        pin_btn.connect_clicked(move |_| {
            p.popdown();
            me.toggle_pin(&id);
        });
        menu.append(&pin_btn);

        let hide_btn = item("Verbergen");
        let (me, p, id) = (self.clone(), pop.clone(), app.id.clone());
        hide_btn.connect_clicked(move |_| {
            p.popdown();
            me.hide_app(&id);
        });
        menu.append(&hide_btn);

        pop.set_child(Some(&menu));
        let me = self.clone();
        pop.connect_closed(move |p| {
            me.menu_open.set(false);
            let p = p.clone();
            me.bar.entry.grab_focus_without_selecting();
            glib::idle_add_local_once(move || p.unparent());
        });
        self.menu_open.set(true);
        pop.popup();
    }

    fn toggle_pin(self: &Rc<Self>, id: &str) {
        {
            let mut ui = self.ui.borrow_mut();
            if ui.state.is_pinned(id) {
                ui.state.unpin(id);
            } else {
                ui.state.pin(id);
            }
        }
        self.refresh(false);
        self.select_app_by_id(id);
    }

    fn hide_app(self: &Rc<Self>, id: &str) {
        self.ui.borrow_mut().state.hide(id);
        self.refresh(false);
    }

    fn select_app_by_id(&self, id: &str) {
        let pos = {
            let mut ui = self.ui.borrow_mut();
            let pos = ui.view_apps.iter().position(|a| a.id == id);
            if let Some(p) = pos {
                ui.selected = p;
            }
            pos
        };
        if let Some(p) = pos {
            self.results.grid.select(p);
        }
    }

    // ---------------------------------------------------------------- bestanden

    fn refresh_files(self: &Rc<Self>, text: &str) {
        let (cwd, hidden) = {
            let mut ui = self.ui.borrow_mut();
            ui.file_gen += 1;
            (ui.files_cwd.clone(), ui.show_hidden)
        };
        self.bar.set_crumb(Some(&util::pretty_path(&cwd)));
        let query = text.trim();
        if query.is_empty() {
            let entries = files::children(&cwd, hidden, &self.cfg().files.ignore);
            self.set_file_rows(entries);
            return;
        }
        let key = (cwd.clone(), hidden);
        let cached = self.ui.borrow().index_cache.get(&key).cloned();
        match cached {
            Some(index) => {
                let gen = self.ui.borrow().file_gen;
                let (tx, q, max) = (
                    self.tx.clone(),
                    query.to_string(),
                    self.cfg().files.max_results,
                );
                self.rt.spawn_blocking(move || {
                    let entries = files::filter(&index, &q, max);
                    let _ = tx.send(Msg::FileResults { gen, entries });
                });
            }
            None => {
                self.results.show_empty("Indexeren…");
                self.results.set_open(true);
                let start = self.ui.borrow_mut().indexing.insert(key);
                if start {
                    let (tx, c) = (self.tx.clone(), self.cfg().files.clone());
                    self.rt.spawn_blocking(move || {
                        let entries =
                            files::scan(&cwd, hidden, c.max_depth, &c.ignore, c.max_index);
                        let _ = tx.send(Msg::FilesIndexed {
                            root: cwd,
                            hidden,
                            entries: Arc::new(entries),
                        });
                    });
                }
            }
        }
    }

    fn set_file_rows(self: &Rc<Self>, entries: Vec<FileEntry>) {
        let rows = entries
            .into_iter()
            .map(|e| ListRow {
                icon: if e.is_dir {
                    "folder"
                } else if e.is_exec {
                    "terminal"
                } else {
                    "file"
                },
                subtitle: util::pretty_path(e.path.parent().unwrap_or(&e.path)),
                title: e.name,
                action: RowAction::Path {
                    path: e.path,
                    is_dir: e.is_dir,
                },
            })
            .collect();
        self.set_rows(rows);
    }

    fn set_rows(self: &Rc<Self>, rows: Vec<ListRow>) {
        let selected = {
            let mut ui = self.ui.borrow_mut();
            ui.rows = rows.clone();
            ui.selected = ui.selected.min(rows.len().saturating_sub(1));
            ui.selected
        };
        if rows.is_empty() {
            self.results.show_empty("Geen resultaten");
        } else {
            let me = self.clone();
            self.results
                .set_rows(&rows, selected, Rc::new(move |i| me.activate_index(i)));
        }
        self.results.set_open(true);
    }

    fn files_up(self: &Rc<Self>) -> bool {
        let moved = {
            let mut ui = self.ui.borrow_mut();
            if ui.files_cwd == ui.files_root {
                false
            } else {
                if let Some(parent) = ui.files_cwd.parent() {
                    ui.files_cwd = parent.to_path_buf();
                }
                ui.selected = 0;
                true
            }
        };
        if moved {
            self.clear_entry();
            self.refresh(true);
        }
        moved
    }

    fn files_enter(self: &Rc<Self>, path: PathBuf) {
        {
            let mut ui = self.ui.borrow_mut();
            ui.files_cwd = path;
            ui.selected = 0;
        }
        self.clear_entry();
        self.refresh(true);
    }

    fn clear_entry(&self) {
        self.suppress.set(true);
        self.bar.entry.set_text("");
        self.suppress.set(false);
    }

    // ---------------------------------------------------------------- browser

    fn ensure_bookmarks(&self) {
        {
            let mut ui = self.ui.borrow_mut();
            if ui
                .bookmarks_at
                .is_some_and(|t| t.elapsed() < Duration::from_secs(60))
            {
                return;
            }
            ui.bookmarks_at = Some(Instant::now());
        }
        let (tx, path) = (self.tx.clone(), self.cfg().browser.bookmarks_path.clone());
        self.rt.spawn_blocking(move || {
            let _ = tx.send(Msg::Bookmarks(bookmarks::load(&path)));
        });
    }

    fn refresh_browser(self: &Rc<Self>, text: &str) {
        self.bar.set_crumb(None);
        let q = text.trim();
        let mut rows: Vec<ListRow> = Vec::new();
        let bookmark_row = |b: &Bookmark| ListRow {
            icon: "bookmark",
            title: b.title.clone(),
            subtitle: b.url.clone(),
            action: RowAction::Url(b.url.clone()),
        };
        {
            let ui = self.ui.borrow();
            if q.is_empty() {
                rows.extend(
                    ui.bookmarks
                        .iter()
                        .take(MAX_BOOKMARK_ROWS)
                        .map(bookmark_row),
                );
            } else {
                rows.push(match browser::looks_like_url(q) {
                    Some(url) => ListRow {
                        icon: "globe",
                        title: format!("Open {url}"),
                        subtitle: "In een nieuw tabblad".into(),
                        action: RowAction::Url(url),
                    },
                    None => ListRow {
                        icon: "search",
                        title: format!("Zoek '{q}' in browser"),
                        subtitle: "In een nieuw tabblad".into(),
                        action: RowAction::Search(q.to_string()),
                    },
                });
                let mut fz = Fuzzy::new(q);
                let mut scored: Vec<(u32, &Bookmark)> = ui
                    .bookmarks
                    .iter()
                    .filter_map(|b| {
                        let t = fz.score(&b.title).map(|s| s.saturating_mul(2));
                        t.max(fz.score(&b.url)).map(|s| (s, b))
                    })
                    .collect();
                scored.sort_by(|a, b| b.0.cmp(&a.0));
                rows.extend(scored.into_iter().take(50).map(|(_, b)| bookmark_row(b)));
            }
        }
        self.set_rows(rows);
    }

    // ---------------------------------------------------------------- wallpapers

    /// Herscant bij elke keer openen van de w-modus; de thumbnail-cache blijft bewaard.
    fn scan_walls(&self) {
        let (tx, dir) = (self.tx.clone(), util::expand_tilde(&self.cfg().wallpaper.dir));
        self.rt.spawn_blocking(move || {
            let _ = tx.send(Msg::Walls(wallpaper::scan(&dir)));
        });
    }

    fn refresh_walls(self: &Rc<Self>, text: &str) {
        self.bar.set_crumb(None);
        let cols = self.cfg().wallpaper.columns.max(1);
        let tile_w = (self.card.width_request() - 28 - 24) / cols as i32;
        let (sections, selected, missing) = {
            let mut ui = self.ui.borrow_mut();
            let Some(all) = ui.walls.as_ref() else {
                drop(ui);
                self.results.show_empty("Laden…");
                self.results.set_open(true);
                return;
            };
            let sections = wallpaper::filter(all, text);
            ui.view_walls = sections.iter().flat_map(|(_, w)| w.iter().cloned()).collect();
            ui.wall_sections = sections.iter().map(|(_, w)| w.len()).collect();
            ui.selected = ui.selected.min(ui.view_walls.len().saturating_sub(1));
            let missing: Vec<PathBuf> = ui
                .view_walls
                .iter()
                .map(|w| w.path.clone())
                .filter(|p| !ui.thumbs.contains_key(p) && !ui.thumbs_loading.contains(p))
                .collect();
            ui.thumbs_loading.extend(missing.iter().cloned());
            (sections, ui.selected, missing)
        };
        if sections.is_empty() {
            let dir = self.cfg().wallpaper.dir.clone();
            self.results.show_empty(&format!("Geen wallpapers in {dir}"));
        } else {
            let me = self.clone();
            self.results.walls.populate(
                &sections,
                cols,
                tile_w,
                selected,
                &self.ui.borrow().thumbs,
                Rc::new(move |i| me.activate_index(i)),
            );
            self.results.show_walls();
        }
        self.results.set_open(true);
        if !missing.is_empty() {
            let tx = self.tx.clone();
            self.rt.spawn_blocking(move || {
                for path in missing {
                    match gdk::gdk_pixbuf::Pixbuf::from_file_at_scale(&path, 480, -1, true) {
                        #[allow(deprecated)]
                        Ok(pb) => {
                            let tex = gdk::Texture::for_pixbuf(&pb);
                            let _ = tx.send(Msg::Thumb { path, tex });
                        }
                        Err(e) => tracing::warn!("thumbnail {} mislukt: {e}", path.display()),
                    }
                }
            });
        }
    }

    fn set_wallpaper(self: &Rc<Self>, keep_open: bool) {
        let wall = {
            let ui = self.ui.borrow();
            ui.view_walls.get(ui.selected).cloned()
        };
        let Some(wall) = wall else { return };
        let cfg = self.cfg().wallpaper.clone();
        self.rt
            .spawn_blocking(move || wallpaper::set(&cfg, &wall.path));
        if !keep_open {
            self.hide();
        }
    }

    // ---------------------------------------------------------------- calculator

    fn refresh_calc(self: &Rc<Self>, text: &str) {
        self.bar.set_crumb(None);
        let ans = self.ui.borrow().state.ans;
        let view = if text.trim().is_empty() {
            let mut ui = self.ui.borrow_mut();
            ui.calc_value = None;
            ui.calc_text = None;
            CalcView::Idle(match ans {
                Some(a) => format!("ans = {}", calc::format(a)),
                None => "sqrt  sin  cos  tan  log  ln  pow  abs  ·  pi  e  ans".into(),
            })
        } else {
            let mut ui = self.ui.borrow_mut();
            match calc::eval(text, ans) {
                Ok(v) => {
                    let s = calc::format(v);
                    ui.calc_value = Some(v);
                    ui.calc_text = Some(s.clone());
                    CalcView::Value {
                        expr: text.trim().to_string(),
                        result: s,
                    }
                }
                Err(e) => {
                    ui.calc_value = None;
                    ui.calc_text = None;
                    CalcView::Error(e)
                }
            }
        };
        self.results.show_calc(view);
        self.results.set_open(true);
    }

    fn calc_copy(self: &Rc<Self>) {
        let expr = self.bar.entry.text().to_string();
        let done = {
            let mut ui = self.ui.borrow_mut();
            match (ui.calc_value, ui.calc_text.clone()) {
                (Some(v), Some(s)) => {
                    util::copy_to_clipboard(&s);
                    ui.state.ans = Some(v);
                    ui.state.history.push(format!("{} = {s}", expr.trim()));
                    true
                }
                _ => false,
            }
        };
        if done {
            self.clear_entry();
            self.refresh(true);
            if self.cfg().calc.close_on_copy {
                self.hide();
            }
        }
    }

    fn calc_insert(self: &Rc<Self>) {
        let text = self.ui.borrow().calc_text.clone();
        if let Some(s) = text {
            self.bar.entry.set_text(&s);
            self.bar.entry.set_position(-1);
        }
    }

    // ---------------------------------------------------------------- acties

    fn activate_index(self: &Rc<Self>, idx: usize) {
        self.ui.borrow_mut().selected = idx;
        self.activate(How::Primary);
    }

    fn activate(self: &Rc<Self>, how: How) {
        let (mode, selected) = {
            let ui = self.ui.borrow();
            (ui.mode, ui.selected)
        };
        match mode {
            Some(Mode::Apps) => {
                let app = self.ui.borrow().view_apps.get(selected).cloned();
                if app.as_ref().is_some_and(|a| a.id == SETTINGS_ID) {
                    self.clear_entry();
                    self.choose_mode(Mode::Settings);
                } else if let Some(app) = app {
                    desktop::launch(&app);
                    self.hide();
                }
            }
            Some(Mode::Files) | Some(Mode::Browser) => {
                let row = self.ui.borrow().rows.get(selected).cloned();
                if let Some(row) = row {
                    self.run_row(row, how);
                }
            }
            Some(Mode::Wallpaper) => self.set_wallpaper(how == How::Manager),
            Some(Mode::Calc) => {
                if how == How::Manager {
                    self.calc_insert();
                } else {
                    self.calc_copy();
                }
            }
            Some(Mode::Settings) | None => {}
        }
    }

    fn run_row(self: &Rc<Self>, row: ListRow, how: How) {
        match row.action {
            RowAction::Path { path, is_dir } => {
                match how {
                    How::Primary if is_dir => {
                        files::open_in_manager(&self.cfg().files, &path, true)
                    }
                    How::Primary => files::open_primary(&self.cfg().files, &path),
                    How::Manager => files::open_in_manager(&self.cfg().files, &path, is_dir),
                    How::Editor => files::open_in_editor(&self.cfg().files, &path),
                }
                self.hide();
            }
            RowAction::Url(url) => {
                browser::open_url(&self.cfg().browser, &url);
                self.hide();
            }
            RowAction::Search(q) => {
                let url = browser::search_url(&self.cfg().browser.search_url, &q);
                browser::open_url(&self.cfg().browser, &url);
                self.hide();
            }
        }
    }

    // ---------------------------------------------------------------- toetsen

    fn on_escape(self: &Rc<Self>) {
        let mode = self.ui.borrow().mode;
        match mode {
            Some(Mode::Files) => {
                if !self.files_up() {
                    self.back_to_general();
                }
            }
            Some(Mode::Calc) | Some(Mode::Browser) | Some(Mode::Wallpaper) => {
                self.back_to_general()
            }
            _ => self.hide(),
        }
    }

    fn move_selection(self: &Rc<Self>, dir: Dir) {
        let mode = self.ui.borrow().mode;
        match mode {
            Some(Mode::Apps) => {
                let new = {
                    let mut ui = self.ui.borrow_mut();
                    let cols = self.app_columns();
                    let sections = [ui.pinned_count, ui.view_apps.len() - ui.pinned_count];
                    ui.selected = grid::navigate(ui.selected, &sections, cols, dir);
                    ui.selected
                };
                self.results.grid.select(new);
            }
            Some(Mode::Wallpaper) => {
                let new = {
                    let mut ui = self.ui.borrow_mut();
                    let cols = self.cfg().wallpaper.columns.max(1);
                    ui.selected = grid::navigate(ui.selected, &ui.wall_sections, cols, dir);
                    ui.selected
                };
                self.results.walls.select(new);
            }
            Some(Mode::Files) | Some(Mode::Browser) => {
                let delta: i32 = match dir {
                    Dir::Up => -1,
                    Dir::Down => 1,
                    _ => 0,
                };
                let new = {
                    let mut ui = self.ui.borrow_mut();
                    let n = ui.rows.len();
                    if n == 0 {
                        return;
                    }
                    ui.selected = (ui.selected as i32 + delta).clamp(0, n as i32 - 1) as usize;
                    ui.selected
                };
                self.results.select_row(new);
            }
            _ => {}
        }
    }

    fn on_key(self: &Rc<Self>, keyval: Key, state: ModifierType) -> glib::Propagation {
        let ctrl = state.contains(ModifierType::CONTROL_MASK);
        let shift = state.contains(ModifierType::SHIFT_MASK);
        let mode = self.ui.borrow().mode;
        let stop = glib::Propagation::Stop;

        // Formulier: Tab/pijlen/Enter horen bij de widgets, alleen Esc gaat terug.
        if mode == Some(Mode::Settings) {
            if keyval == Key::Escape {
                self.back_to_general();
                return stop;
            }
            return glib::Propagation::Proceed;
        }

        match keyval {
            Key::Escape => {
                self.on_escape();
                stop
            }
            Key::BackSpace
                if self.bar.entry.text().is_empty()
                    && matches!(
                        mode,
                        Some(Mode::Files)
                            | Some(Mode::Calc)
                            | Some(Mode::Browser)
                            | Some(Mode::Wallpaper)
                    ) =>
            {
                if mode == Some(Mode::Files) {
                    if !self.files_up() {
                        self.back_to_general();
                    }
                } else {
                    self.back_to_general();
                }
                stop
            }
            Key::Return | Key::KP_Enter | Key::ISO_Enter => {
                self.activate(if ctrl {
                    How::Manager
                } else if shift {
                    How::Editor
                } else {
                    How::Primary
                });
                stop
            }
            Key::Tab | Key::ISO_Left_Tab => {
                if mode == Some(Mode::Calc) {
                    self.calc_insert();
                }
                stop
            }
            Key::Down => {
                self.move_selection(Dir::Down);
                stop
            }
            Key::Up => {
                self.move_selection(Dir::Up);
                stop
            }
            Key::Left => match mode {
                Some(Mode::Apps) | Some(Mode::Wallpaper) => {
                    self.move_selection(Dir::Left);
                    stop
                }
                Some(Mode::Files) if self.bar.entry.position() == 0 => {
                    if !self.files_up() {
                        self.back_to_general();
                    }
                    stop
                }
                Some(Mode::Calc) | Some(Mode::Browser) if self.bar.entry.position() == 0 => {
                    self.back_to_general();
                    stop
                }
                _ => glib::Propagation::Proceed,
            },
            Key::Right => match mode {
                Some(Mode::Apps) | Some(Mode::Wallpaper) => {
                    self.move_selection(Dir::Right);
                    stop
                }
                Some(Mode::Files) => {
                    let at_end =
                        self.bar.entry.position() as usize >= self.bar.entry.text().chars().count();
                    let target = {
                        let ui = self.ui.borrow();
                        match ui.rows.get(ui.selected).map(|r| &r.action) {
                            Some(RowAction::Path { path, is_dir: true }) => Some(path.clone()),
                            _ => None,
                        }
                    };
                    match (at_end, target) {
                        (true, Some(path)) => {
                            self.files_enter(path);
                            stop
                        }
                        _ => glib::Propagation::Proceed,
                    }
                }
                _ => glib::Propagation::Proceed,
            },
            Key::h | Key::H if ctrl && mode == Some(Mode::Files) => {
                {
                    let mut ui = self.ui.borrow_mut();
                    ui.show_hidden = !ui.show_hidden;
                }
                self.refresh(true);
                stop
            }
            Key::p | Key::P if ctrl && mode == Some(Mode::Apps) => {
                let id = {
                    let ui = self.ui.borrow();
                    ui.view_apps.get(ui.selected).map(|a| a.id.clone())
                };
                if let Some(id) = id {
                    self.toggle_pin(&id);
                }
                stop
            }
            _ => glib::Propagation::Proceed,
        }
    }

    // ---------------------------------------------------------------- achtergrondberichten

    pub fn prewarm(self: &Rc<Self>) {
        if self.ui.borrow().visible {
            return;
        }
        self.window.set_keyboard_mode(KeyboardMode::None);
        self.window.set_opacity(0.0);
        self.window.set_visible(true);
        let me = self.clone();
        glib::timeout_add_local_once(Duration::from_millis(250), move || {
            if !me.ui.borrow().visible {
                me.window.set_visible(false);
            }
            me.window.set_opacity(1.0);
            me.window.set_keyboard_mode(KeyboardMode::Exclusive);
        });
    }

    fn handle(self: &Rc<Self>, msg: Msg) {
        match msg {
            Msg::Apps(list) => {
                self.ui.borrow_mut().apps = list;
                if self.ui.borrow().mode == Some(Mode::Apps) {
                    self.refresh(false);
                }
            }
            Msg::Bookmarks(list) => {
                self.ui.borrow_mut().bookmarks = list;
                if self.ui.borrow().mode == Some(Mode::Browser) {
                    self.refresh(false);
                }
            }
            Msg::FilesIndexed {
                root,
                hidden,
                entries,
            } => {
                let current = {
                    let mut ui = self.ui.borrow_mut();
                    ui.indexing.remove(&(root.clone(), hidden));
                    ui.index_cache.insert((root.clone(), hidden), entries);
                    ui.mode == Some(Mode::Files) && ui.files_cwd == root && ui.show_hidden == hidden
                };
                if current {
                    self.refresh(false);
                }
            }
            Msg::Walls(sections) => {
                self.ui.borrow_mut().walls = Some(sections);
                if self.ui.borrow().mode == Some(Mode::Wallpaper) {
                    self.refresh(false);
                }
            }
            Msg::Thumb { path, tex } => {
                self.results.walls.set_thumb(&path, &tex);
                let mut ui = self.ui.borrow_mut();
                ui.thumbs_loading.remove(&path);
                ui.thumbs.insert(path, tex);
            }
            Msg::FileResults { gen, entries } => {
                let fresh = {
                    let ui = self.ui.borrow();
                    ui.file_gen == gen && ui.mode == Some(Mode::Files)
                };
                if fresh {
                    self.set_file_rows(entries);
                }
            }
        }
    }
}
