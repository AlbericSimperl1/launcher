use gtk::glib::object::ObjectType;
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{
    wl_compositor::WlCompositor, wl_region::WlRegion, wl_registry::WlRegistry,
    wl_surface::WlSurface,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1,
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
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
    ExtBackgroundEffectManagerV1, ();
    ExtBackgroundEffectSurfaceV1, ();
    WlRegistry, GlobalListContents;
);

pub struct Blur {
    conn: Connection,
    qh: QueueHandle<State>,
    _queue: EventQueue<State>,
    compositor: WlCompositor,
    manager: ExtBackgroundEffectManagerV1,
    effect: RefCell<Option<ExtBackgroundEffectSurfaceV1>>,
    last: Cell<(i32, i32, i32, i32, i32)>,
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
        let manager: ExtBackgroundEffectManagerV1 = globals.bind(&qh, 1..=1, ()).ok()?;
        let _ = queue.roundtrip(&mut State);
        Some(Self {
            conn,
            qh,
            _queue: queue,
            compositor,
            manager,
            effect: RefCell::new(None),
            last: Cell::new((0, 0, 0, 0, 0)),
        })
    }

    pub fn attach(&self, window: &gtk::ApplicationWindow) {
        if let Some(old) = self.effect.borrow_mut().take() {
            old.destroy();
        }
        self.last.set((0, 0, 0, 0, 0));
        let Some(surface) = window.surface() else {
            return;
        };
        let sptr = unsafe {
            gdk4_wayland::ffi::gdk_wayland_surface_get_wl_surface(surface.as_ptr() as *mut _)
        } as *mut c_void;
        if sptr.is_null() {
            return;
        }
        let Ok(id) = (unsafe { ObjectId::from_ptr(WlSurface::interface(), sptr.cast()) }) else {
            return;
        };
        let Ok(wl) = WlSurface::from_id(&self.conn, id) else {
            return;
        };
        *self.effect.borrow_mut() = Some(self.manager.get_background_effect(&wl, &self.qh, ()));
        let _ = self.conn.flush();
    }

    pub fn update(&self, x: i32, y: i32, w: i32, h: i32, radius: i32) {
        let key = (x, y, w, h, radius);
        if self.last.get() == key || w <= 0 || h <= 0 {
            return;
        }
        let effect = self.effect.borrow();
        let Some(effect) = effect.as_ref() else {
            return;
        };
        self.last.set(key);
        let r = radius.min(w / 2).min(h / 2).max(0);
        let region = self.compositor.create_region(&self.qh, ());
        for row in 0..r {
            let dy = (r - row) as f64 - 0.5;
            let inset = r - ((r as f64).powi(2) - dy.powi(2)).max(0.0).sqrt().round() as i32;
            region.add(x + inset, y + row, w - 2 * inset, 1);
            region.add(x + inset, y + h - 1 - row, w - 2 * inset, 1);
        }
        if h > 2 * r {
            region.add(x, y + r, w, h - 2 * r);
        }
        effect.set_blur_region(Some(&region));
        region.destroy();
        let _ = self.conn.flush();
    }
}
