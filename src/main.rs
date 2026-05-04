mod app;
mod config;
mod core;
mod error;
mod i18n;
mod lang;
mod services;
mod util;
mod windows;

use log::info;
use relm4::{gtk, RelmApp};

use crate::app::AppMsg;
use std::path::PathBuf;
use std::sync::Arc;

use crate::config::AppConfig;

const APP_DESKTOP_ID: &str = "com.pot-app.pot-gtk";

fn main() {
    env_logger::init();
    info!("Starting pot-gtk");

    crate::core::runtime::init();

    let config = Arc::new(AppConfig::new());
    if let Some(action) = cli_action_arg() {
        if let Err(e) = crate::core::http_server::send_local_action(&config, &action) {
            eprintln!("Failed to send action '{}': {}", action, e);
            std::process::exit(1);
        }
        return;
    }

    // Set LANGUAGE env var from config BEFORE i18n::init(), otherwise
    // bindtextdomain/textdomain cache the messages with the wrong locale.
    if let Some(lang) = config
        .get("app_language")
        .and_then(|v| v.as_str().map(String::from))
    {
        if lang != "en" && lang != "auto" && !lang.is_empty() {
            i18n::set_language(&lang);
        }
    }

    i18n::init();

    // RelmApp::new calls gtk::init() + adw::init() internally
    let app: RelmApp<AppMsg> = RelmApp::new(APP_DESKTOP_ID);

    // Apply theme (GTK/Adwaita now initialized)
    let theme = config
        .get("app_theme")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "system".into());
    i18n::apply_theme(&theme);

    // Load global CSS
    let display = gtk::gdk::Display::default().expect("Could not connect to a display");
    setup_app_icon(&display);
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../data/style.css"));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Don't auto-show the root ApplicationWindow (AppModel's window is a hidden container)
    app.visible_on_activate(false).run::<app::AppModel>(config);
}

fn setup_app_icon(display: &gtk::gdk::Display) {
    gtk::glib::set_application_name("Pot GTK");
    gtk::glib::set_prgname(Some(APP_DESKTOP_ID));
    gtk::Window::set_default_icon_name(APP_DESKTOP_ID);

    let icon_theme = gtk::IconTheme::for_display(display);
    for path in icon_search_paths() {
        if path.exists() {
            icon_theme.add_search_path(path);
        }
    }
}

fn icon_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut roots = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("icons"));
        }
    }

    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/icons"));

    for root in roots {
        paths.push(root.clone());
        for subdir in [
            "scalable/apps",
            "128x128/apps",
            "64x64/apps",
            "48x48/apps",
            "32x32/apps",
            "16x16/apps",
        ] {
            paths.push(root.join(subdir));
        }
    }

    paths
}

fn cli_action_arg() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--pot-action" || arg == "--action" {
            return args.next();
        }
        if let Some(value) = arg.strip_prefix("--pot-action=") {
            return Some(value.to_string());
        }
        if let Some(value) = arg.strip_prefix("--action=") {
            return Some(value.to_string());
        }
    }
    None
}
