// src/blur.rs — volledige vervanging
// GDK claimt zelf al een ext_background_effect op de venster-surface (leeg = geen blur),
// een tweede object op dezelfde surface is een protocolfout. Daarom: eigen subsurface
// onder het venster, en daar het blur-effect op zetten.

use gtk::glib::object::ObjectType;
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::fs::File;
use std::io::Write;
use std::os::fd::AsFd;
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{
    wl_buffer::WlBuffer,
    wl_compositor::WlCompositor,
    wl_region::WlRegion,
    wl_registry::WlRegistry,
    wl_shm::{self, WlShm},
    wl_shm_pool::WlShmPool,
    wl_subcompositor::WlSubcompositor,
    wl_subsurface::WlSubsurface,
    wl_surface::WlSurface,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1,
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
};
use wayland_protocols::wp::viewporter::client::{
    wp_viewport::WpViewport, wp_viewporter::WpViewporter,
};

struct State;

macro_rules! noop {
    ($($proxy:ty, $data:ty);* $(;)?) => { $(
        impl Dispatch<$proxy, $data> for State {
            fn event(
                _: &mut Self,
                _: &$proxy,
                _: <$proxy as Proxy>::Event,
                _: &$data,
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {}
        }
    )* };
}

noop!(
    WlCompositor, ();
    WlRegion, ();
    WlSurface, ();
    WlSubcompositor, ();
    WlSubsurface, ();
    WlShm, ();
    WlShmPool, ();
    WlBuffer, ();
    WpViewporter, ();
    WpViewport, ();
    ExtBackgroundEffectManagerV1, ();
    ExtBackgroundEffectSurfaceV1, ();
    WlRegistry, GlobalListContents;
);

struct Overlay {
    surf: WlSurface,
    sub: WlSubsurface,
    vp: WpViewport,
    effect: ExtBackgroundEffectSurfaceV1,
}

pub struct Blur {
    conn: Connection,
    qh: QueueHandle<State>,
    queue: RefCell<EventQueue<State>>,
    compositor: WlCompositor,
    subcomp: WlSubcompositor,
    viewporter: WpViewporter,
    manager: ExtBackgroundEffectManagerV1,
    buffer: WlBuffer,
    _shm_file: File,
    overlay: RefCell<Option<Overlay>>,
    parent: Cell<usize>,
    last: Cell<(i32, i32, i32, i32, i32)>,
    dead: Cell<bool>,
}

fn shm_buffer(shm: &WlShm, qh: &QueueHandle<State>) -> Option<(WlBuffer, File)> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = dir.join(format!("rl-blur-{}", std::process::id()));
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .ok()?;
    let _ = std::fs::remove_file(&path);
    // 1x1 ARGB8888 (little endian: B G R A), alpha 1/255: praktisch onzichtbaar.
    f.write_all(&[0, 0, 0, 1]).ok()?;
    let pool = shm.create_pool(f.as_fd(), 4, qh, ());
    let buf = pool.create_buffer(0, 1, 1, 4, wl_shm::Format::Argb8888, qh, ());
    pool.destroy();
    Some((buf, f))
}

impl Blur {
    pub fn new() -> Option<Self> {
        let display = gtk::gdk::Display::default()?;
        let dptr = unsafe {
            gdk4_wayland::ffi::gdk_wayland_display_get_wl_display(display.as_ptr() as *mut _)
        } as *mut c_void;
        if dptr.is_null() {
            return None;
        }
        let backend = unsafe { Backend::from_foreign_display(dptr.cast()) };
        let conn = Connection::from_backend(backend);
        let (globals, mut queue) = registry_queue_init::<State>(&conn).ok()?;
        let qh = queue.handle();
        let compositor: WlCompositor = globals.bind(&qh, 1..=6, ()).ok()?;
        let subcomp: WlSubcompositor = globals.bind(&qh, 1..=1, ()).ok()?;
        let viewporter: WpViewporter = globals.bind(&qh, 1..=1, ()).ok()?;
        let shm: WlShm = globals.bind(&qh, 1..=1, ()).ok()?;
        let manager: ExtBackgroundEffectManagerV1 = globals.bind(&qh, 1..=1, ()).ok()?;
        let (buffer, file) = shm_buffer(&shm, &qh)?;
        queue.roundtrip(&mut State).ok()?;
        Some(Self {
            conn,
            qh,
            queue: RefCell::new(queue),
            compositor,
            subcomp,
            viewporter,
            manager,
            buffer,
            _shm_file: file,
            overlay: RefCell::new(None),
            parent: Cell::new(0),
            last: Cell::new((0, 0, 0, 0, 0)),
            dead: Cell::new(false),
        })
    }

    fn pump(&self) -> bool {
        if self.dead.get() {
            return false;
        }
        let res = self
            .queue
            .borrow_mut()
            .dispatch_pending(&mut State)
            .and_then(|_| self.conn.flush().map_err(|e| e.into()));
        if let Err(e) = res {
            tracing::warn!("blur-protocol uitgeschakeld: {e}");
            self.dead.set(true);
            self.parent.set(0);
            *self.overlay.borrow_mut() = None;
            return false;
        }
        true
    }

    pub fn is_dead(&self) -> bool {
        self.dead.get()
    }

    pub fn detach(&self) {
        self.last.set((0, 0, 0, 0, 0));
        self.parent.set(0);
        let old = self.overlay.borrow_mut().take();
        if let Some(o) = old {
            if !self.dead.get() {
                o.effect.destroy();
                o.vp.destroy();
                o.sub.destroy();
                o.surf.destroy();
            }
        }
        self.pump();
    }

    fn attach(&self, sptr: *mut c_void) {
        self.detach();
        if self.dead.get() {
            return;
        }
        let Ok(id) = (unsafe { ObjectId::from_ptr(WlSurface::interface(), sptr.cast()) }) else {
            return;
        };
        let Ok(parent) = WlSurface::from_id(&self.conn, id) else {
            return;
        };
        let qh = &self.qh;
        let surf = self.compositor.create_surface(qh, ());
        let empty = self.compositor.create_region(qh, ());
        surf.set_input_region(Some(&empty));
        empty.destroy();
        let sub = self.subcomp.get_subsurface(&surf, &parent, qh, ());
        sub.place_below(&parent);
        sub.set_desync();
        let vp = self.viewporter.get_viewport(&surf, qh, ());
        let effect = self.manager.get_background_effect(&surf, qh, ());
        surf.attach(Some(&self.buffer), 0, 0);
        *self.overlay.borrow_mut() = Some(Overlay {
            surf,
            sub,
            vp,
            effect,
        });
        self.parent.set(sptr as usize);
        self.pump();
    }

    /// Geeft `true` als er iets veranderd is (dan moet GTK het venster opnieuw committen).
    pub fn sync(
        &self,
        window: &gtk::ApplicationWindow,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        radius: i32,
    ) -> bool {
        if self.dead.get() {
            return false;
        }
        if !window.is_mapped() {
            if self.parent.get() != 0 {
                self.detach();
            }
            return false;
        }
        let Some(surface) = window.surface() else {
            return false;
        };
        let sptr = unsafe {
            gdk4_wayland::ffi::gdk_wayland_surface_get_wl_surface(surface.as_ptr() as *mut _)
        } as *mut c_void;
        if sptr.is_null() {
            self.detach();
            return false;
        }
        if self.parent.get() != sptr as usize {
            self.attach(sptr);
        }
        self.update(x, y, w, h, radius)
    }

    fn update(&self, x: i32, y: i32, w: i32, h: i32, radius: i32) -> bool {
        let key = (x, y, w, h, radius);
        if self.dead.get() || self.last.get() == key || w <= 0 || h <= 0 {
            return false;
        }
        let overlay = self.overlay.borrow();
        let Some(o) = overlay.as_ref() else {
            return false;
        };
        self.last.set(key);

        o.sub.set_position(x, y);
        o.vp.set_destination(w, h);

        let r = radius.clamp(0, w / 2).min(h / 2);
        let region = self.compositor.create_region(&self.qh, ());
        for row in 0..r {
            let dy = (r - row) as f64 - 0.5;
            let inset = r - ((r as f64).powi(2) - dy.powi(2)).max(0.0).sqrt().round() as i32;
            let rw = w - 2 * inset;
            if rw > 0 {
                region.add(inset, row, rw, 1);
                region.add(inset, h - 1 - row, rw, 1);
            }
        }
        if h > 2 * r {
            region.add(0, r, w, h - 2 * r);
        }
        o.effect.set_blur_region(Some(&region));
        region.destroy();
        o.surf.damage(0, 0, i32::MAX, i32::MAX);
        o.surf.commit();
        drop(overlay);
        self.pump()
    }
}
