//! Entrypoint: GTK-applicatie met single-instance gedrag (daemon + toggle via D-Bus).

mod app;
mod bookmarks;
mod config;
mod desktop;
mod modes;
mod state;
mod ui;
mod util;

use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{gio, glib};
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
    Help,
}

fn parse(args: &[String]) -> Cmd {
    for a in args {
        match a.as_str() {
            "--daemon" | "-d" => return Cmd::Daemon,
            "--show" => return Cmd::Show,
            "--hide" => return Cmd::Hide,
            "--unhide-all" => return Cmd::UnhideAll,
            "--help" | "-h" => return Cmd::Help,
            _ => {}
        }
    }
    Cmd::Toggle
}

fn main() -> glib::ExitCode {
    if std::env::args().skip(1).any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return glib::ExitCode::SUCCESS;
    }
    let _log_guard = util::init_logging();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("rl-worker")
        .build()
        .expect("tokio runtime");
    let handle = runtime.handle().clone();

    let application = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

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
                let l = app::Launcher::new(application, config::load(), handle.clone(), tx);
                l.spawn_receiver(rx);
                std::mem::forget(application.hold());
                tracing::info!("launcher gestart");
                *slot = Some(l);
            }
            slot.as_ref().cloned().expect("launcher")
        };

        match cmd {
            Cmd::Show => instance.show(),
            Cmd::Hide => instance.hide(),
            Cmd::Toggle => instance.toggle(),
            Cmd::UnhideAll => instance.unhide_all(),
            Cmd::Daemon | Cmd::Help => {}
        }
        0
    });

    application.run()
}
