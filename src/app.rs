use gtk::prelude::*;
use relm4::prelude::*;
use std::sync::Arc;

use crate::config::AppConfig;
use crate::core::clipboard::{ClipboardEvent, ClipboardMonitor};
use crate::core::history::HistoryStore;
use crate::core::http_server::{self, ActionEvent, AppAction};
use crate::core::tray::SystemTray;
use crate::services::translate;
use crate::windows::config::{ConfigModel, ConfigOutput};
use crate::windows::recognize::{RecognizeModel, RecognizeMsg, RecognizeOutput};
use crate::windows::screenshot::{ScreenshotModel, ScreenshotMsg, ScreenshotOutput};
use crate::windows::translate::{TranslateModel, TranslateMsg, TranslateOutput};
use crate::windows::updater::UpdaterModel;

#[allow(dead_code)]
pub struct AppModel {
    config: Arc<AppConfig>,
    #[allow(dead_code)]
    registry: Arc<translate::TranslateRegistry>,
    #[allow(dead_code)]
    tray: SystemTray,
    clipboard_monitor: ClipboardMonitor,
    history: Arc<HistoryStore>,
    translate_controller: Controller<TranslateModel>,
    config_controller: Controller<ConfigModel>,
    updater_controller: Controller<UpdaterModel>,
    screenshot_controller: Controller<ScreenshotModel>,
    recognize_controller: Controller<RecognizeModel>,
}

#[derive(Debug)]
pub enum AppMsg {
    Action(ActionEvent),
    #[allow(dead_code)]
    ClipboardEvent(ClipboardEvent),
    ScreenshotCaptured,
    ScreenshotCancelled,
    RecognizeTranslate(String),
    TranslateClosed,
    RecognizeClosed,
    LanguageChanged(String),
}

#[relm4::component(pub)]
impl Component for AppModel {
    type Init = Arc<AppConfig>;
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = ();

    view! {
        adw::ApplicationWindow {
            set_visible: false,
            set_icon_name: Some("com.pot-app.pot-gtk"),
        }
    }

    fn init(
        config: Arc<AppConfig>,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let input_sender = sender.input_sender().clone();

        // Bridge AppAction from background threads to main thread
        let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel::<ActionEvent>();
        let input_sender_clone = input_sender.clone();
        let bridge_handle = crate::core::runtime::handle().expect("shared runtime not initialized");
        std::thread::spawn(move || {
            bridge_handle.block_on(async {
                while let Some(action) = action_rx.recv().await {
                    let sender = input_sender_clone.clone();
                    gtk4::glib::idle_add_once(move || {
                        let _ = sender.send(AppMsg::Action(action));
                    });
                }
            });
        });

        // Start HTTP server
        let port = config
            .get("server_port")
            .map(|v| v.as_i64().unwrap_or(60828))
            .unwrap_or(60828);
        let _server_handle = http_server::start_server(port, action_tx.clone(), config.clone());

        // Set proxy
        if config
            .get("proxy_enable")
            .map(|v| v.as_bool().unwrap_or(false))
            .unwrap_or(false)
        {
            if let Err(e) = crate::core::proxy::set_proxy(&config) {
                log::warn!("Proxy setup failed: {}", e);
            }
        }

        // Create plugin manager and service registries
        let plugin_manager = crate::services::plugin::PluginManager::new();
        let registry = Arc::new(translate::create_registry_with_plugins(&plugin_manager));
        let tts_registry = Arc::new(crate::services::tts::create_registry());
        let collection_registry = Arc::new(crate::services::collection::create_registry());
        let recognize_registry = Arc::new(crate::services::recognize::create_registry());

        let tray = SystemTray::new(action_tx.clone());
        let clipboard_monitor = ClipboardMonitor::new();

        // Start clipboard monitor
        {
            let input_sender_clone = input_sender.clone();
            let (clip_tx, mut clip_rx) = tokio::sync::mpsc::unbounded_channel::<ClipboardEvent>();
            clipboard_monitor.start(clip_tx);
            let clip_handle =
                crate::core::runtime::handle().expect("shared runtime not initialized");
            std::thread::spawn(move || {
                clip_handle.block_on(async {
                    while let Some(event) = clip_rx.recv().await {
                        let sender = input_sender_clone.clone();
                        gtk4::glib::idle_add_once(move || {
                            let _ = sender.send(AppMsg::ClipboardEvent(event));
                        });
                    }
                });
            });
        }
        let clipboard_enabled = config
            .get("clipboard_translate")
            .map(|v| v.as_bool().unwrap_or(false))
            .unwrap_or(false);
        clipboard_monitor.set_enabled(clipboard_enabled);

        // Register global hotkeys (X11, feature-gated)
        #[cfg(feature = "hotkey")]
        {
            if let Err(e) = crate::core::hotkey::register_all(&config, action_tx.clone()) {
                log::warn!("Failed to register hotkeys: {}", e);
            }
        }

        // Open history database
        let history = Arc::new(HistoryStore::open().unwrap_or_else(|e| {
            log::warn!("Failed to open history database (attempt 1): {}", e);
            HistoryStore::open().unwrap_or_else(|e2| {
                log::error!("Failed to open history database (attempt 2): {}", e2);
                HistoryStore::open_in_memory().expect("Failed to create in-memory history database")
            })
        }));

        let translate_controller = TranslateModel::builder()
            .launch((
                config.clone(),
                registry.clone(),
                tts_registry.clone(),
                collection_registry.clone(),
                history.clone(),
            ))
            .forward(sender.input_sender(), |output| match output {
                TranslateOutput::Closed => AppMsg::TranslateClosed,
            });

        let config_controller = ConfigModel::builder()
            .launch((
                config.clone(),
                registry.clone(),
                recognize_registry.clone(),
                tts_registry.clone(),
                collection_registry.clone(),
            ))
            .forward(sender.input_sender(), |output| match output {
                ConfigOutput::LanguageChanged(lang) => AppMsg::LanguageChanged(lang),
            });

        let updater_controller = UpdaterModel::builder().launch(()).detach();

        let screenshot_controller =
            ScreenshotModel::builder()
                .launch(())
                .forward(sender.input_sender(), |output| match output {
                    ScreenshotOutput::Captured => AppMsg::ScreenshotCaptured,
                    ScreenshotOutput::Cancelled => AppMsg::ScreenshotCancelled,
                });

        let recognize_controller = RecognizeModel::builder()
            .launch((recognize_registry, config.clone()))
            .forward(sender.input_sender(), |output| match output {
                RecognizeOutput::TranslateText(text) => AppMsg::RecognizeTranslate(text),
                RecognizeOutput::Closed => AppMsg::RecognizeClosed,
            });

        if config.is_first_run() {
            config_controller.widget().present();
        } else {
            translate_controller.widget().present();
        }

        let model = AppModel {
            config,
            registry,
            tray,
            clipboard_monitor,
            history,
            translate_controller,
            config_controller,
            updater_controller,
            screenshot_controller,
            recognize_controller,
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            AppMsg::Action(event) => {
                let ctx = event.ctx;
                match event.action {
                    AppAction::TranslateText(text) => {
                        self.translate_controller.emit(TranslateMsg::Show(text));
                        self.position_translate_window(&ctx);
                    }
                    AppAction::InputTranslate => {
                        self.translate_controller
                            .emit(TranslateMsg::Show(String::new()));
                        self.position_translate_window(&ctx);
                    }
                    AppAction::SelectionTranslate => {
                        let text = selection::get_text();
                        self.translate_controller.emit(TranslateMsg::Show(text));
                        self.position_translate_window(&ctx);
                    }
                    AppAction::OcrRecognize { screenshot } => {
                        if screenshot {
                            self.screenshot_controller
                                .emit(ScreenshotMsg::StartCapture(0, 0));
                        } else {
                            self.recognize_controller.emit(RecognizeMsg::Show);
                            self.recognize_controller.widget().present();
                        }
                    }
                    AppAction::OcrTranslate { screenshot } => {
                        if screenshot {
                            self.screenshot_controller
                                .emit(ScreenshotMsg::StartCapture(0, 0));
                        } else {
                            self.recognize_controller.emit(RecognizeMsg::Show);
                            self.recognize_controller.widget().present();
                        }
                    }
                    AppAction::ShowConfig => {
                        self.config_controller.widget().present();
                    }
                    AppAction::ShowUpdater => {
                        self.updater_controller.widget().present();
                    }
                    AppAction::Quit => {
                        relm4::main_application().quit();
                    }
                }
            }
            AppMsg::ClipboardEvent(ClipboardEvent::NewText(text)) => {
                if self.clipboard_monitor.is_enabled() {
                    self.translate_controller.emit(TranslateMsg::Show(text));
                    self.position_translate_window(&Default::default());
                }
            }
            AppMsg::ScreenshotCaptured => {
                self.recognize_controller.emit(RecognizeMsg::Show);
                self.recognize_controller.widget().present();
            }
            AppMsg::ScreenshotCancelled => {}
            AppMsg::RecognizeTranslate(text) => {
                self.translate_controller.emit(TranslateMsg::Show(text));
                self.position_translate_window(&Default::default());
            }
            AppMsg::TranslateClosed => {
                self.translate_controller.widget().set_visible(false);
            }
            AppMsg::RecognizeClosed => {
                self.recognize_controller.widget().set_visible(false);
            }
            AppMsg::LanguageChanged(lang) => {
                self.tray.update_language(&lang);
            }
        }
    }
}

impl AppModel {
    fn position_translate_window(&self, ctx: &crate::core::http_server::ActivationContext) {
        let win = self.translate_controller.widget();
        win.set_default_size(400, 500);

        let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        match session_type.as_str() {
            "wayland" => {
                if let Some(ref token) = ctx.startup_id {
                    win.set_startup_id(token);
                } else {
                    log::debug!(
                        "No activation token available on Wayland; compositor will place window"
                    );
                }
                if let Some(ts) = ctx.timestamp {
                    win.present_with_time((ts & 0xFFFFFFFF) as u32);
                } else {
                    win.present();
                }
            }
            "x11" => {
                #[cfg(feature = "hotkey")]
                {
                    self.position_x11_near_cursor(&win);
                }
                #[cfg(not(feature = "hotkey"))]
                {
                    win.present();
                }
            }
            _ => {
                win.present();
            }
        }
    }

    #[cfg(feature = "hotkey")]
    fn position_x11_near_cursor(&self, win: &gtk::Window) {
        use mouse_position::mouse_position::Mouse;
        match Mouse::get_mouse_position() {
            Mouse::Position { x, y } => {
                let w = win.default_width();
                let h = win.default_height();
                let target_x = (x - w / 2).max(0);
                let target_y = (y - h / 2).max(0);

                // Schedule the move after the window is mapped
                let win_clone = win.clone();
                let map_handler = win_clone.connect_map(move |_w| {
                    move_x11_window_to(target_x, target_y);
                });

                win.present();

                // Disconnect the handler after it fires to avoid re-moving
                // on subsequent map events.
                let win_clone2 = win.clone();
                gtk4::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(500),
                    move || {
                        win_clone2.disconnect(map_handler);
                    },
                );
            }
            _ => {
                win.present();
            }
        }
    }
}

#[cfg(feature = "hotkey")]
fn move_x11_window_to(x: i32, y: i32) {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConfigureWindowAux;
    use x11rb::rust_connection::RustConnection;

    // Find the most recently mapped window belonging to our PID.
    // We search in reverse order (children are newest-first in X11)
    // to prefer the top-level window that was just presented.
    let xid = find_our_x11_window();
    if let Some(xid) = xid {
        if let Ok((conn, _)) = RustConnection::connect(None) {
            let aux = ConfigureWindowAux::new().x(x).y(y);
            let _ = x11rb::protocol::xproto::configure_window(&conn, xid, &aux);
            let _ = conn.flush();
        }
    } else {
        log::debug!("Could not determine X11 window ID, skipping move");
    }
}

/// Find this process's top-level X11 window by searching for _NET_WM_PID.
/// Limits recursion depth to prevent stack overflow on large window trees.
#[cfg(feature = "hotkey")]
fn find_our_x11_window() -> Option<u32> {
    use x11rb::connection::Connection;
    use x11rb::rust_connection::RustConnection;

    let (conn, screen_num) = RustConnection::connect(None).ok()?;
    let setup = conn.setup();
    let root = setup.roots.get(screen_num)?.root;
    let target_pid = std::process::id();

    let pid_atom = x11rb::protocol::xproto::intern_atom(&conn, false, b"_NET_WM_PID")
        .ok()?
        .reply()
        .ok()?
        .atom;

    search_pid(&conn, root, pid_atom, target_pid, 10)
}

#[cfg(feature = "hotkey")]
fn search_pid(
    conn: &x11rb::rust_connection::RustConnection,
    win: u32,
    pid_atom: u32,
    target_pid: u32,
    depth: u32,
) -> Option<u32> {
    if depth == 0 {
        return None;
    }

    // Check this window's _NET_WM_PID
    if let Ok(cookie) = x11rb::protocol::xproto::get_property(
        conn,
        false,
        win,
        pid_atom,
        x11rb::protocol::xproto::AtomEnum::CARDINAL,
        0,
        1,
    ) {
        if let Ok(reply) = cookie.reply() {
            if reply.value.len() >= 4 {
                let pid = u32::from_ne_bytes([
                    reply.value[0],
                    reply.value[1],
                    reply.value[2],
                    reply.value[3],
                ]);
                if pid == target_pid {
                    return Some(win);
                }
            }
        }
    }

    // Recurse into children (reversed to prefer last-mapped / top-level windows)
    if let Ok(cookie) = x11rb::protocol::xproto::query_tree(conn, win) {
        if let Ok(tree) = cookie.reply() {
            for &child in tree.children.iter().rev() {
                if let Some(found) = search_pid(conn, child, pid_atom, target_pid, depth - 1) {
                    return Some(found);
                }
            }
        }
    }

    None
}
