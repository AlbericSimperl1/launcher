//! Entrypoint: GTK-applicatie met single-instance gedrag (daemon + toggle via D-Bus).

mod app;
mod bookmarks;
mod config;
mod desktop;
mod modes;
mod state;
mod ui;
mod util;

use gtk::prelude::*;
use gtk::{gio, glib};
use gtk4 as gtk;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc;

const APP_ID: &str = "dev.rustlauncher.Launcher";

const HELP: &str = "rust-launcher — frosted glass launcher voor Wayland/Hyprland

Gebruik: rust-launcher [OPTIE]
  (geen)         toon of verberg de launcher (toggle)
  --daemon, -d   start op de achtergrond zonder te tonen
  --show         toon de launcher
  --hide         verberg de launcher
  --toggle       toon of verberg de launcher
  --unhide-all   maak alle verborgen apps weer zichtbaar
  --help, -h     toon deze hulp
";

#[derive(Clone, Copy)]
enum Cmd {
    Daemon,
    Show,
    Hide,
    Toggle,
    UnhideAll,
    Reload,
    Help,
}

fn parse(args: &[String]) -> Cmd {
    for a in args {
        match a.as_str() {
            "--daemon" | "-d" => return Cmd::Daemon,
            "--show" => return Cmd::Show,
            "--hide" => return Cmd::Hide,
            "--unhide-all" => return Cmd::UnhideAll,
            "--reload" => return Cmd::Reload, // ← nieuw

            "--help" | "-h" => return Cmd::Help,
            _ => {}
        }
    }
    Cmd::Toggle
}

use std::os::unix::process::CommandExt;

fn ensure_layer_shell_preload() {
    if std::env::var_os("RL_PRELOADED").is_some() {
        return;
    }
    let candidates = [
        "/usr/lib/libgtk4-layer-shell.so",
        "/usr/lib64/libgtk4-layer-shell.so",
        "/usr/lib/x86_64-linux-gnu/libgtk4-layer-shell.so",
        "/usr/local/lib/libgtk4-layer-shell.so",
    ];
    let Some(lib) = candidates.iter().find(|p| std::path::Path::new(p).exists()) else {
        return;
    };
    let preload = match std::env::var("LD_PRELOAD") {
        Ok(old) if !old.is_empty() => format!("{lib}:{old}"),
        _ => lib.to_string(),
    };
    let err = std::process::Command::new(std::env::current_exe().expect("exe"))
        .args(std::env::args().skip(1))
        .env("LD_PRELOAD", preload)
        .env("RL_PRELOADED", "1")
        .exec();
    eprintln!("re-exec mislukt: {err}");
}

// src/main.rs — begin of main(), vóór GTK/tokio threads
fn main() -> glib::ExitCode {
    // src/main.rs — in main(), direct na `fn main() -> glib::ExitCode {`
    glib::log_set_handler(
        Some("Gdk"),
        glib::LogLevels::LEVEL_WARNING,
        false,
        false,
        |domain, level, msg| {
            // Onschadelijke GTK4-melding bij snel tonen/verbergen van het venster
            if msg.contains("gdk_frame_timings_throttling_hint") {
                return;
            }
            glib::log_default_handler(domain, level, Some(msg));
        },
    );
    std::env::remove_var("LD_PRELOAD");
    if std::env::var_os("XDG_CURRENT_DESKTOP").is_none() {
        std::env::set_var("XDG_CURRENT_DESKTOP", "Hyprland");
    }

    if std::env::args().skip(1).any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return glib::ExitCode::SUCCESS;
    }
    // ... rest ongewijzigd

    let application = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    let _ = application.register(gio::Cancellable::NONE);
    let primary = !application.is_remote();

    let _log_guard = if primary { util::init_logging() } else { None };
    let runtime = primary.then(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("rl-worker")
            .build()
            .expect("tokio runtime")
    });
    let handle = runtime.as_ref().map(|r| r.handle().clone());

    let launcher: Rc<RefCell<Option<Rc<app::Launcher>>>> = Rc::default();

    application.connect_command_line(move |application, cmdline| {
        let args: Vec<String> = cmdline
            .arguments()
            .iter()
            .skip(1)
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let cmd = parse(&args);

        let instance = {
            let mut slot = launcher.borrow_mut();
            if slot.is_none() {
                let (tx, rx) = mpsc::unbounded_channel();
                let l = app::Launcher::new(
                    application,
                    config::load(),
                    handle.clone().expect("runtime"),
                    tx,
                );
                l.spawn_receiver(rx);
                std::mem::forget(application.hold());
                tracing::info!("launcher gestart");
                *slot = Some(l);
            }
            slot.as_ref().cloned().expect("launcher")
        };

        match cmd {
            Cmd::Daemon => instance.prewarm(),
            Cmd::Help => {}
            Cmd::Show => instance.show(),
            Cmd::Hide => instance.hide(),
            Cmd::Reload => instance.reload(),

            Cmd::Toggle => instance.toggle(),
            Cmd::UnhideAll => instance.unhide_all(),
            Cmd::Daemon | Cmd::Help => {}
        }
        0
    });

    application.run()
}
