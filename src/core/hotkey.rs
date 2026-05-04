use crate::config::AppConfig;
use crate::core::http_server::{ActionEvent, AppAction};
use crate::error::AppError;
use log::{info, warn};
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[cfg(feature = "hotkey")]
use x11rb::protocol::xproto::*;

const PORTAL_APP_ID: &str = "com.pot_app.pot_gtk";
const WAYLAND_DUPLICATE_HOTKEY_WINDOW: Duration = Duration::from_millis(800);

#[derive(Debug, Clone)]
pub struct HotkeyStatus {
    pub session_type: String,
    pub backend: String,
    pub detail: String,
    pub ok: bool,
}

pub fn status(config: &AppConfig) -> HotkeyStatus {
    let session_type = detect_session_type();
    let shortcuts = load_shortcut_config(config);
    let configured = shortcuts
        .iter()
        .filter(|(shortcut, _, _)| !shortcut.trim().is_empty())
        .count();

    match session_type.as_str() {
        "wayland" => wayland_status(&shortcuts, configured),
        "x11" => HotkeyStatus {
            session_type,
            backend: "X11".into(),
            detail: format!("{} shortcut(s) configured", configured),
            ok: true,
        },
        _ => HotkeyStatus {
            session_type: if session_type.is_empty() {
                "unknown".into()
            } else {
                session_type
            },
            backend: "Auto".into(),
            detail: "Session type could not be detected; Pot will try X11 first and then Wayland."
                .into(),
            ok: false,
        },
    }
}

/// Register all global hotkeys. Automatically picks the right backend
/// based on the current session type (X11 or Wayland).
pub fn register_all(
    config: &AppConfig,
    action_tx: mpsc::UnboundedSender<ActionEvent>,
) -> Result<(), AppError> {
    let session_type = detect_session_type();
    info!("Session type: {}", session_type);

    match session_type.as_str() {
        "x11" => register_x11(config, action_tx),
        "wayland" => register_wayland(config, action_tx),
        _ => {
            // Unknown — try X11 first, fall back to Wayland
            info!("Unknown session type, trying X11 then Wayland");
            if register_x11(config, action_tx.clone()).is_err() {
                register_wayland(config, action_tx)
            } else {
                Ok(())
            }
        }
    }
}

fn detect_session_type() -> String {
    let session_type = std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_lowercase();
    if session_type == "x11" || session_type == "wayland" {
        return session_type;
    }

    if std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        "wayland".to_string()
    } else if std::env::var("DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        "x11".to_string()
    } else {
        session_type
    }
}

/// Re-register shortcuts after config change.
pub fn register_shortcut(
    _name: &str,
    _shortcut: &str,
    _action_tx: &mpsc::UnboundedSender<ActionEvent>,
) -> Result<(), AppError> {
    // Full re-registration is handled by restarting the backend.
    info!("Shortcut re-registration requested — requires restart to take effect");
    Ok(())
}

/// Unregister all hotkeys and stop the backend event loop.
#[allow(dead_code)]
pub fn unregister_all() {
    // The event loop thread exits when the sender is dropped.
    // For a clean shutdown, the caller drops the action_tx channel.
    info!("Hotkey unregistration requested");
}

// ── Shortcut config ─────────────────────────────────────────────────────────

struct ShortcutDef {
    config_key: &'static str,
    default_key: &'static str,
    label: &'static str,
    action: AppAction,
}

fn get_shortcut_defs() -> Vec<ShortcutDef> {
    vec![
        ShortcutDef {
            config_key: "hotkey_selection_translate",
            default_key: "Ctrl+Shift+S",
            label: "Selection Translate",
            action: AppAction::SelectionTranslate,
        },
        ShortcutDef {
            config_key: "hotkey_input_translate",
            default_key: "Ctrl+Shift+I",
            label: "Input Translate",
            action: AppAction::InputTranslate,
        },
        ShortcutDef {
            config_key: "hotkey_ocr_recognize",
            default_key: "Ctrl+Shift+O",
            label: "OCR Recognize",
            action: AppAction::OcrRecognize { screenshot: true },
        },
        ShortcutDef {
            config_key: "hotkey_ocr_translate",
            default_key: "Ctrl+Shift+T",
            label: "OCR Translate",
            action: AppAction::OcrTranslate { screenshot: true },
        },
    ]
}

fn load_shortcut_config(config: &AppConfig) -> Vec<(String, String, AppAction)> {
    get_shortcut_defs()
        .into_iter()
        .map(|def| {
            let key = config
                .get(def.config_key)
                .and_then(|v| v.as_str().map(String::from))
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| def.default_key.to_string());
            (key, def.label.to_string(), def.action)
        })
        .collect()
}

#[derive(Clone)]
struct HotkeyDispatcher {
    action_tx: mpsc::UnboundedSender<ActionEvent>,
    last_sent: Arc<Mutex<HashMap<&'static str, Instant>>>,
}

impl HotkeyDispatcher {
    fn new(action_tx: mpsc::UnboundedSender<ActionEvent>) -> Self {
        Self {
            action_tx,
            last_sent: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn send(&self, event: ActionEvent) {
        let key = action_key(&event.action);
        let now = Instant::now();

        {
            let mut last_sent = self.last_sent.lock().unwrap_or_else(|e| e.into_inner());
            if last_sent
                .get(key)
                .is_some_and(|last| now.duration_since(*last) < WAYLAND_DUPLICATE_HOTKEY_WINDOW)
            {
                return;
            }
            last_sent.insert(key, now);
        }

        let _ = self.action_tx.send(event);
    }
}

fn action_key(action: &AppAction) -> &'static str {
    match action {
        AppAction::TranslateText(_) => "translate_text",
        AppAction::InputTranslate => "input_translate",
        AppAction::SelectionTranslate => "selection_translate",
        AppAction::OcrRecognize { screenshot: true } => "ocr_recognize_screenshot",
        AppAction::OcrRecognize { screenshot: false } => "ocr_recognize",
        AppAction::OcrTranslate { screenshot: true } => "ocr_translate_screenshot",
        AppAction::OcrTranslate { screenshot: false } => "ocr_translate",
        AppAction::ShowConfig => "show_config",
        AppAction::ShowUpdater => "show_updater",
        AppAction::Quit => "quit",
    }
}

// ── X11 backend ──────────────────────────────────────────────────────────────

fn register_x11(
    config: &AppConfig,
    action_tx: mpsc::UnboundedSender<ActionEvent>,
) -> Result<(), AppError> {
    let shortcuts = load_shortcut_config(config);

    std::thread::spawn(move || {
        if let Err(e) = run_x11_loop(shortcuts, action_tx) {
            warn!("X11 hotkey backend failed: {}", e);
        }
    });

    Ok(())
}

fn run_x11_loop(
    shortcuts: Vec<(String, String, AppAction)>,
    action_tx: mpsc::UnboundedSender<ActionEvent>,
) -> Result<(), AppError> {
    use x11rb::connection::Connection;
    use x11rb::protocol::Event;
    use x11rb::rust_connection::RustConnection;

    let (conn, _screen_num) = RustConnection::connect(None)
        .map_err(|e| AppError::Custom(format!("X11 connection failed: {}", e)))?;
    let screen = &conn.setup().roots[0];
    let root = screen.root;

    let mut key_map: HashMap<(ModMask, Keycode), AppAction> = HashMap::new();

    if let Ok(keycodes) = get_keycodes(&conn) {
        for (key_str, _label, action) in &shortcuts {
            if key_str.is_empty() {
                continue;
            }
            if let Some((mod_mask, keycode)) = parse_x11_shortcut(key_str, &keycodes) {
                match do_grab_key(&conn, root, mod_mask, keycode) {
                    Ok(()) => {
                        key_map.insert((mod_mask, keycode), action.clone());
                    }
                    Err(e) => warn!("Failed to grab X11 hotkey '{}': {}", key_str, e),
                }
            }
        }
    }

    let _ = conn.flush();
    info!("X11 hotkey backend started with {} bindings", key_map.len());

    loop {
        match conn.wait_for_event() {
            Ok(event) => {
                if let Event::KeyPress(kp) = event {
                    let ignored_masks =
                        u16::from(ModMask::M2) | u16::from(ModMask::M5) | u16::from(ModMask::LOCK);
                    let state = ModMask::from(u16::from(kp.state) & !ignored_masks);
                    if let Some(action) = key_map.get(&(state, kp.detail)) {
                        let _ = action_tx.send(ActionEvent::new(action.clone()));
                    }
                }
            }
            Err(e) => {
                warn!("X11 event loop error: {}", e);
                break;
            }
        }
    }
    Ok(())
}

struct KeyCodeMap {
    min: u8,
    names: Vec<Vec<String>>,
}

fn get_keycodes(conn: &x11rb::rust_connection::RustConnection) -> Result<KeyCodeMap, AppError> {
    use x11rb::connection::Connection;
    let setup = conn.setup();
    let min = setup.min_keycode;
    let max = setup.max_keycode;
    let reply = get_keyboard_mapping(conn, min, max - min + 1)
        .map_err(|e| AppError::Custom(e.to_string()))?
        .reply()
        .map_err(|e| AppError::Custom(e.to_string()))?;

    let keysyms_per_keycode = reply.keysyms_per_keycode as usize;
    let mut names = Vec::new();
    for chunk in reply.keysyms.chunks(keysyms_per_keycode) {
        let mut key_names = Vec::new();
        for &sym in chunk {
            key_names.push(keysym_to_string(sym));
        }
        names.push(key_names);
    }

    Ok(KeyCodeMap { min, names })
}

fn parse_x11_shortcut(shortcut: &str, keycodes: &KeyCodeMap) -> Option<(ModMask, Keycode)> {
    let parts: Vec<&str> = shortcut.split('+').collect();
    let key_name = parts.last()?;
    let mut mod_mask = ModMask::from(0u8);

    for &part in &parts[..parts.len() - 1] {
        match part.trim().to_lowercase().as_str() {
            "ctrl" | "control" | "cmdorctrl" | "commandorcontrol" => {
                mod_mask = ModMask::from(u16::from(mod_mask) | u16::from(ModMask::CONTROL))
            }
            "alt" | "option" => {
                mod_mask = ModMask::from(u16::from(mod_mask) | u16::from(ModMask::M1))
            }
            "shift" => mod_mask = ModMask::from(u16::from(mod_mask) | u16::from(ModMask::SHIFT)),
            "super" | "win" | "meta" => {
                mod_mask = ModMask::from(u16::from(mod_mask) | u16::from(ModMask::M4))
            }
            _ => {}
        }
    }

    let target = key_name.trim().to_lowercase();
    for (i, key_names) in keycodes.names.iter().enumerate() {
        for name in key_names {
            if name.to_lowercase() == target {
                return Some((mod_mask, keycodes.min + i as u8));
            }
        }
    }

    let target_upper = key_name.trim().to_uppercase();
    for (i, key_names) in keycodes.names.iter().enumerate() {
        for name in key_names {
            if name == &target_upper {
                return Some((mod_mask, keycodes.min + i as u8));
            }
        }
    }

    warn!("Could not find keycode for key: {}", key_name);
    None
}

fn do_grab_key(
    conn: &x11rb::rust_connection::RustConnection,
    root: Window,
    modifiers: ModMask,
    keycode: Keycode,
) -> Result<(), AppError> {
    grab_key(
        conn,
        false,
        root,
        modifiers,
        keycode,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
    )
    .map_err(|e| AppError::Custom(e.to_string()))?;

    let lock_combinations = [
        u16::from(ModMask::M2),
        u16::from(ModMask::LOCK),
        u16::from(ModMask::M5),
        u16::from(ModMask::M2) | u16::from(ModMask::LOCK),
        u16::from(ModMask::M2) | u16::from(ModMask::M5),
        u16::from(ModMask::LOCK) | u16::from(ModMask::M5),
        u16::from(ModMask::M2) | u16::from(ModMask::LOCK) | u16::from(ModMask::M5),
    ];
    for extra in lock_combinations {
        let combined = ModMask::from(u16::from(modifiers) | u16::from(extra));
        let _ = grab_key(
            conn,
            false,
            root,
            combined,
            keycode,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        );
    }
    Ok(())
}

fn keysym_to_string(keysym: u32) -> String {
    match keysym {
        0x0061..=0x007a => ((keysym - 32) as u8 as char).to_string(),
        0x0041..=0x005a => (keysym as u8 as char).to_string(),
        0x0030..=0x0039 => (keysym as u8 as char).to_string(),
        0xff08 => "Backspace".to_string(),
        0xff09 => "Tab".to_string(),
        0xff0d => "Return".to_string(),
        0xff1b => "Escape".to_string(),
        0xff50 => "Home".to_string(),
        0xff51 => "Left".to_string(),
        0xff52 => "Up".to_string(),
        0xff53 => "Right".to_string(),
        0xff54 => "Down".to_string(),
        0xff55 => "Page_Up".to_string(),
        0xff56 => "Page_Down".to_string(),
        0xff57 => "End".to_string(),
        0xff63 => "Insert".to_string(),
        0xffff => "Delete".to_string(),
        0xffbe..=0xffd0 => format!("F{}", keysym - 0xffbe + 1),
        0xffe1 => "Shift_L".to_string(),
        0xffe2 => "Shift_R".to_string(),
        0xffe3 => "Control_L".to_string(),
        0xffe4 => "Control_R".to_string(),
        0xffe9 => "Alt_L".to_string(),
        0xffea => "Alt_R".to_string(),
        0xffeb => "Super_L".to_string(),
        0xffec => "Super_R".to_string(),
        0x0020 => "space".to_string(),
        _ => String::new(),
    }
}

// ── Wayland backend (ashpd GlobalShortcuts portal) ───────────────────────────

fn register_wayland(
    config: &AppConfig,
    action_tx: mpsc::UnboundedSender<ActionEvent>,
) -> Result<(), AppError> {
    let shortcuts = load_shortcut_config(config);
    let evdev_shortcuts = shortcuts.clone();
    let gnome_shortcuts = shortcuts.clone();
    let dispatcher = HotkeyDispatcher::new(action_tx);
    let portal_dispatcher = dispatcher.clone();
    let evdev_dispatcher = dispatcher.clone();

    if is_gnome_desktop() {
        if let Err(e) = register_gnome_custom_shortcuts(&gnome_shortcuts) {
            warn!("GNOME custom hotkey backend failed: {}", e);
        }
    }

    std::thread::spawn(move || {
        if let Err(e) = run_wayland_loop(shortcuts, portal_dispatcher) {
            warn!("Wayland hotkey backend failed: {}", e);
        }
    });

    std::thread::spawn(move || {
        if let Err(e) = run_evdev_loop(evdev_shortcuts, evdev_dispatcher) {
            warn!("Wayland evdev hotkey fallback failed: {}", e);
        }
    });

    Ok(())
}

fn run_wayland_loop(
    shortcuts: Vec<(String, String, AppAction)>,
    dispatcher: HotkeyDispatcher,
) -> Result<(), AppError> {
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
    use futures_util::StreamExt;

    let handle = crate::core::runtime::handle()
        .ok_or_else(|| AppError::Custom("shared tokio runtime not initialized".into()))?;
    handle.block_on(async {
        register_portal_host_app().await;

        let proxy = GlobalShortcuts::new()
            .await
            .map_err(|e| AppError::Custom(format!("GlobalShortcuts portal: {}", e)))?;

        let session = proxy
            .create_session()
            .await
            .map_err(|e| AppError::Custom(format!("create session: {}", e)))?;

        let shortcut_defs: Vec<NewShortcut> = shortcuts
            .iter()
            .enumerate()
            .map(|(i, (key, label, _action))| {
                let trigger =
                    shortcut_to_portal_trigger(key).unwrap_or_else(|| key.to_string());
                NewShortcut::new(format!("shortcut_{}", i), label.clone())
                    .preferred_trigger(trigger.as_str())
            })
            .collect();

        let bind_response = proxy
            .bind_shortcuts(&session, &shortcut_defs, None)
            .await
            .map_err(|e| AppError::Custom(format!("bind_shortcuts: {}", e)))?
            .response()
            .map_err(|e| AppError::Custom(format!("bind_shortcuts response: {}", e)))?;

        for shortcut in bind_response.shortcuts() {
            info!(
                "Wayland shortcut bound: id={}, trigger={}",
                shortcut.id(),
                shortcut.trigger_description()
            );
        }

        if bind_response.shortcuts().is_empty() {
            return Err(AppError::Custom(
                "Wayland GlobalShortcuts portal returned no bound shortcuts; check desktop portal support and app permissions"
                    .into(),
            ));
        }

        info!(
            "Wayland GlobalShortcuts backend started with {} bindings",
            bind_response.shortcuts().len()
        );

        let mut stream = proxy
            .receive_activated()
            .await
            .map_err(|e| AppError::Custom(format!("receive_activated: {}", e)))?;

        while let Some(activated) = stream.next().await {
            let raw_id = activated.shortcut_id();
            let idx: usize = match raw_id.trim_start_matches("shortcut_").parse() {
                Ok(i) => i,
                Err(_) => {
                    warn!("Ignoring malformed shortcut ID: {}", raw_id);
                    continue;
                }
            };

            if let Some((_, _, action)) = shortcuts.get(idx) {
                let token = extract_activation_token(activated.options());
                let ts = activated.timestamp().as_millis() as u64;

                let mut event = ActionEvent::new(action.clone());
                if let Some(t) = token {
                    event = event.with_startup_id(t);
                }
                event = event.with_timestamp(ts);

                dispatcher.send(event);
            }
        }

        Ok(())
    })
}

const GNOME_MEDIA_KEYS_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const GNOME_CUSTOM_KEYBINDING_SCHEMA: &str =
    "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const GNOME_CUSTOM_KEYBINDING_BASE: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/";

fn is_gnome_desktop() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    let session = std::env::var("DESKTOP_SESSION")
        .unwrap_or_default()
        .to_lowercase();
    desktop.contains("gnome") || session.contains("gnome") || session.contains("ubuntu")
}

fn wayland_status(shortcuts: &[(String, String, AppAction)], configured: usize) -> HotkeyStatus {
    if is_gnome_desktop() && command_available("gsettings") {
        let convertible = shortcuts
            .iter()
            .filter(|(shortcut, _, action)| {
                action_cli_arg(action).is_some() && shortcut_to_gnome_binding(shortcut).is_some()
            })
            .count();
        return HotkeyStatus {
            session_type: "wayland".into(),
            backend: "GNOME custom keybindings".into(),
            detail: format!(
                "{} of {} shortcut(s) can be registered through GNOME Settings.",
                convertible, configured
            ),
            ok: convertible > 0,
        };
    }

    if global_shortcuts_portal_available() {
        return HotkeyStatus {
            session_type: "wayland".into(),
            backend: "xdg-desktop-portal GlobalShortcuts".into(),
            detail: format!("{} shortcut(s) configured", configured),
            ok: true,
        };
    }

    let evdev_count = readable_keyboard_device_count();
    if evdev_count > 0 {
        return HotkeyStatus {
            session_type: "wayland".into(),
            backend: "evdev /dev/input fallback".into(),
            detail: format!("{} readable keyboard device(s) found", evdev_count),
            ok: true,
        };
    }

    HotkeyStatus {
        session_type: "wayland".into(),
        backend: "No available backend".into(),
        detail:
            "Install a portal frontend with GlobalShortcuts support, use GNOME custom shortcuts, or grant /dev/input read permission."
                .into(),
        ok: false,
    }
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn global_shortcuts_portal_available() -> bool {
    let output = Command::new("gdbus")
        .args([
            "introspect",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
        ])
        .output();
    output
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .contains("org.freedesktop.portal.GlobalShortcuts")
        })
        .unwrap_or(false)
}

fn readable_keyboard_device_count() -> usize {
    let Ok(entries) = std::fs::read_dir("/dev/input") else {
        return 0;
    };

    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name().and_then(|s| s.to_str())?;
            if !name.starts_with("event") {
                return None;
            }
            let device = evdev::Device::open(&path).ok()?;
            if device.supported_keys().map_or(false, looks_like_keyboard) {
                Some(())
            } else {
                None
            }
        })
        .count()
}

fn register_gnome_custom_shortcuts(
    shortcuts: &[(String, String, AppAction)],
) -> Result<(), AppError> {
    if Command::new("gsettings").arg("--version").output().is_err() {
        return Err(AppError::Custom("gsettings command not found".into()));
    }

    let exe = std::env::current_exe()
        .map_err(|e| AppError::Custom(format!("Cannot resolve current executable: {}", e)))?;
    let exe = shell_quote(&exe.to_string_lossy());
    let current = Command::new("gsettings")
        .args(["get", GNOME_MEDIA_KEYS_SCHEMA, "custom-keybindings"])
        .output()
        .map_err(|e| AppError::Custom(format!("Failed to read GNOME keybindings: {}", e)))?;
    if !current.status.success() {
        return Err(AppError::Custom(format!(
            "gsettings get custom-keybindings failed: {}",
            String::from_utf8_lossy(&current.stderr)
        )));
    }

    let mut paths = parse_gsettings_path_list(&String::from_utf8_lossy(&current.stdout));
    let mut registered = 0usize;

    for (shortcut, label, action) in shortcuts {
        let Some(action_arg) = action_cli_arg(action) else {
            continue;
        };
        let Some(binding) = shortcut_to_gnome_binding(shortcut) else {
            warn!("Cannot convert GNOME shortcut '{}': {}", label, shortcut);
            continue;
        };
        let Some(id) = gnome_binding_id(action) else {
            continue;
        };

        let path = format!("{}{}/", GNOME_CUSTOM_KEYBINDING_BASE, id);
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path.clone());
        }

        let schema_path = format!("{}:{}", GNOME_CUSTOM_KEYBINDING_SCHEMA, path);
        let command = format!("{} --pot-action {}", exe, action_arg);
        run_gsettings_set(&schema_path, "name", &format!("Pot {}", label))?;
        run_gsettings_set(&schema_path, "command", &command)?;
        run_gsettings_set(&schema_path, "binding", &binding)?;
        registered += 1;
    }

    run_gsettings_set(
        GNOME_MEDIA_KEYS_SCHEMA,
        "custom-keybindings",
        &format_gsettings_path_list(&paths),
    )?;
    info!(
        "GNOME custom hotkey backend registered {} binding(s)",
        registered
    );
    Ok(())
}

fn run_gsettings_set(schema: &str, key: &str, value: &str) -> Result<(), AppError> {
    let output = Command::new("gsettings")
        .args(["set", schema, key, value])
        .output()
        .map_err(|e| AppError::Custom(format!("Failed to run gsettings: {}", e)))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::Custom(format!(
            "gsettings set {} {} failed: {}",
            schema,
            key,
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

fn parse_gsettings_path_list(raw: &str) -> Vec<String> {
    raw.split('\'')
        .enumerate()
        .filter_map(|(idx, value)| {
            if idx % 2 == 1 && !value.is_empty() {
                Some(value.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn format_gsettings_path_list(paths: &[String]) -> String {
    let values = paths
        .iter()
        .map(|path| format!("'{}'", path.replace('\'', "\\'")))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{}]", values)
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn gnome_binding_id(action: &AppAction) -> Option<&'static str> {
    match action {
        AppAction::SelectionTranslate => Some("pot-gtk-selection-translate"),
        AppAction::InputTranslate => Some("pot-gtk-input-translate"),
        AppAction::OcrRecognize { screenshot: true } => Some("pot-gtk-ocr-recognize"),
        AppAction::OcrTranslate { screenshot: true } => Some("pot-gtk-ocr-translate"),
        _ => None,
    }
}

fn action_cli_arg(action: &AppAction) -> Option<&'static str> {
    match action {
        AppAction::SelectionTranslate => Some("selection-translate"),
        AppAction::InputTranslate => Some("input-translate"),
        AppAction::OcrRecognize { screenshot: true } => Some("ocr-recognize"),
        AppAction::OcrTranslate { screenshot: true } => Some("ocr-translate"),
        _ => None,
    }
}

fn shortcut_to_gnome_binding(shortcut: &str) -> Option<String> {
    let parts: Vec<&str> = shortcut
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let key = parts.last()?;
    let mut binding = String::new();

    for part in &parts[..parts.len().saturating_sub(1)] {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" | "cmdorctrl" | "commandorcontrol" => binding.push_str("<Control>"),
            "shift" => binding.push_str("<Shift>"),
            "alt" | "option" => binding.push_str("<Alt>"),
            "super" | "win" | "meta" => binding.push_str("<Super>"),
            _ => return None,
        }
    }

    binding.push_str(&normalize_gnome_key(key));
    Some(binding)
}

fn normalize_gnome_key(key: &str) -> String {
    match key.trim().to_lowercase().as_str() {
        "esc" => "Escape".to_string(),
        "return" | "enter" => "Return".to_string(),
        "space" => "space".to_string(),
        other if other.len() == 1 => other.to_string(),
        _ => key.trim().to_string(),
    }
}

#[derive(Clone)]
struct EvdevShortcut {
    key: evdev::KeyCode,
    ctrl: bool,
    shift: bool,
    alt: bool,
    logo: bool,
    action: AppAction,
}

fn run_evdev_loop(
    shortcuts: Vec<(String, String, AppAction)>,
    dispatcher: HotkeyDispatcher,
) -> Result<(), AppError> {
    let shortcuts: Vec<EvdevShortcut> = shortcuts
        .into_iter()
        .filter_map(|(shortcut, label, action)| {
            let parsed = parse_evdev_shortcut(&shortcut, action);
            if parsed.is_none() {
                warn!("Cannot parse evdev shortcut '{}': {}", label, shortcut);
            }
            parsed
        })
        .collect();
    if shortcuts.is_empty() {
        return Err(AppError::Custom("No evdev shortcuts configured".into()));
    }

    let mut started = 0usize;
    let entries = std::fs::read_dir("/dev/input")
        .map_err(|e| AppError::Custom(format!("Cannot read /dev/input: {}", e)))?;

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.starts_with("event") {
            continue;
        }

        let Ok(device) = evdev::Device::open(&path) else {
            continue;
        };
        if !device.supported_keys().map_or(false, looks_like_keyboard) {
            continue;
        }

        let device_name = device.name().unwrap_or("unknown keyboard").to_string();
        let dispatcher = dispatcher.clone();
        let shortcuts = shortcuts.clone();
        std::thread::spawn(move || {
            run_evdev_device_loop(device, device_name, shortcuts, dispatcher)
        });
        started += 1;
    }

    if started == 0 {
        return Err(AppError::Custom(
            "No readable keyboard devices found under /dev/input. Add the user to the input group or use a desktop that supports the GlobalShortcuts portal.".into(),
        ));
    }

    info!(
        "Wayland evdev hotkey fallback started on {} keyboard device(s)",
        started
    );

    loop {
        std::thread::park();
    }
}

fn looks_like_keyboard(keys: &evdev::AttributeSetRef<evdev::KeyCode>) -> bool {
    keys.contains(evdev::KeyCode::KEY_A)
        && keys.contains(evdev::KeyCode::KEY_Z)
        && keys.contains(evdev::KeyCode::KEY_ENTER)
}

fn run_evdev_device_loop(
    mut device: evdev::Device,
    device_name: String,
    shortcuts: Vec<EvdevShortcut>,
    dispatcher: HotkeyDispatcher,
) {
    info!("Listening for Wayland evdev hotkeys on {}", device_name);
    let mut pressed = HashSet::new();

    loop {
        match device.fetch_events() {
            Ok(events) => {
                for event in events {
                    if let evdev::EventSummary::Key(_, code, value) = event.destructure() {
                        match value {
                            0 => {
                                pressed.remove(&code);
                            }
                            1 => {
                                pressed.insert(code);
                                for shortcut in matching_evdev_shortcuts(&pressed, code, &shortcuts)
                                {
                                    dispatcher.send(ActionEvent::new(shortcut.action.clone()));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Err(e) => {
                warn!("evdev hotkey listener stopped for {}: {}", device_name, e);
                break;
            }
        }
    }
}

fn matching_evdev_shortcuts<'a>(
    pressed: &'a HashSet<evdev::KeyCode>,
    key: evdev::KeyCode,
    shortcuts: &'a [EvdevShortcut],
) -> impl Iterator<Item = &'a EvdevShortcut> + 'a {
    shortcuts.iter().filter(move |shortcut| {
        shortcut.key == key
            && shortcut.ctrl
                == any_pressed(
                    pressed,
                    &[evdev::KeyCode::KEY_LEFTCTRL, evdev::KeyCode::KEY_RIGHTCTRL],
                )
            && shortcut.shift
                == any_pressed(
                    pressed,
                    &[
                        evdev::KeyCode::KEY_LEFTSHIFT,
                        evdev::KeyCode::KEY_RIGHTSHIFT,
                    ],
                )
            && shortcut.alt
                == any_pressed(
                    pressed,
                    &[evdev::KeyCode::KEY_LEFTALT, evdev::KeyCode::KEY_RIGHTALT],
                )
            && shortcut.logo
                == any_pressed(
                    pressed,
                    &[evdev::KeyCode::KEY_LEFTMETA, evdev::KeyCode::KEY_RIGHTMETA],
                )
    })
}

fn any_pressed(pressed: &HashSet<evdev::KeyCode>, keys: &[evdev::KeyCode]) -> bool {
    keys.iter().any(|key| pressed.contains(key))
}

fn parse_evdev_shortcut(shortcut: &str, action: AppAction) -> Option<EvdevShortcut> {
    let parts: Vec<&str> = shortcut
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let key = evdev_key(parts.last()?)?;

    let mut parsed = EvdevShortcut {
        key,
        ctrl: false,
        shift: false,
        alt: false,
        logo: false,
        action,
    };

    for part in &parts[..parts.len().saturating_sub(1)] {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" | "cmdorctrl" | "commandorcontrol" => parsed.ctrl = true,
            "shift" => parsed.shift = true,
            "alt" | "option" => parsed.alt = true,
            "super" | "win" | "meta" => parsed.logo = true,
            _ => return None,
        }
    }

    Some(parsed)
}

fn evdev_key(key: &str) -> Option<evdev::KeyCode> {
    let key = key.trim().to_lowercase();
    let code = match key.as_str() {
        "a" => evdev::KeyCode::KEY_A,
        "b" => evdev::KeyCode::KEY_B,
        "c" => evdev::KeyCode::KEY_C,
        "d" => evdev::KeyCode::KEY_D,
        "e" => evdev::KeyCode::KEY_E,
        "f" => evdev::KeyCode::KEY_F,
        "g" => evdev::KeyCode::KEY_G,
        "h" => evdev::KeyCode::KEY_H,
        "i" => evdev::KeyCode::KEY_I,
        "j" => evdev::KeyCode::KEY_J,
        "k" => evdev::KeyCode::KEY_K,
        "l" => evdev::KeyCode::KEY_L,
        "m" => evdev::KeyCode::KEY_M,
        "n" => evdev::KeyCode::KEY_N,
        "o" => evdev::KeyCode::KEY_O,
        "p" => evdev::KeyCode::KEY_P,
        "q" => evdev::KeyCode::KEY_Q,
        "r" => evdev::KeyCode::KEY_R,
        "s" => evdev::KeyCode::KEY_S,
        "t" => evdev::KeyCode::KEY_T,
        "u" => evdev::KeyCode::KEY_U,
        "v" => evdev::KeyCode::KEY_V,
        "w" => evdev::KeyCode::KEY_W,
        "x" => evdev::KeyCode::KEY_X,
        "y" => evdev::KeyCode::KEY_Y,
        "z" => evdev::KeyCode::KEY_Z,
        "0" => evdev::KeyCode::KEY_0,
        "1" => evdev::KeyCode::KEY_1,
        "2" => evdev::KeyCode::KEY_2,
        "3" => evdev::KeyCode::KEY_3,
        "4" => evdev::KeyCode::KEY_4,
        "5" => evdev::KeyCode::KEY_5,
        "6" => evdev::KeyCode::KEY_6,
        "7" => evdev::KeyCode::KEY_7,
        "8" => evdev::KeyCode::KEY_8,
        "9" => evdev::KeyCode::KEY_9,
        "f1" => evdev::KeyCode::KEY_F1,
        "f2" => evdev::KeyCode::KEY_F2,
        "f3" => evdev::KeyCode::KEY_F3,
        "f4" => evdev::KeyCode::KEY_F4,
        "f5" => evdev::KeyCode::KEY_F5,
        "f6" => evdev::KeyCode::KEY_F6,
        "f7" => evdev::KeyCode::KEY_F7,
        "f8" => evdev::KeyCode::KEY_F8,
        "f9" => evdev::KeyCode::KEY_F9,
        "f10" => evdev::KeyCode::KEY_F10,
        "f11" => evdev::KeyCode::KEY_F11,
        "f12" => evdev::KeyCode::KEY_F12,
        "return" | "enter" => evdev::KeyCode::KEY_ENTER,
        "escape" | "esc" => evdev::KeyCode::KEY_ESC,
        "space" => evdev::KeyCode::KEY_SPACE,
        "tab" => evdev::KeyCode::KEY_TAB,
        "backspace" => evdev::KeyCode::KEY_BACKSPACE,
        "delete" => evdev::KeyCode::KEY_DELETE,
        "insert" => evdev::KeyCode::KEY_INSERT,
        "home" => evdev::KeyCode::KEY_HOME,
        "end" => evdev::KeyCode::KEY_END,
        "pageup" | "page_up" => evdev::KeyCode::KEY_PAGEUP,
        "pagedown" | "page_down" => evdev::KeyCode::KEY_PAGEDOWN,
        "left" => evdev::KeyCode::KEY_LEFT,
        "right" => evdev::KeyCode::KEY_RIGHT,
        "up" => evdev::KeyCode::KEY_UP,
        "down" => evdev::KeyCode::KEY_DOWN,
        _ => return None,
    };
    Some(code)
}

async fn register_portal_host_app() {
    match PORTAL_APP_ID.parse::<ashpd::AppID>() {
        Ok(app_id) => {
            if let Err(e) = ashpd::register_host_app(app_id).await {
                warn!("Failed to register host app with xdg portal: {}", e);
            }
        }
        Err(e) => warn!("Invalid portal app id '{}': {}", PORTAL_APP_ID, e),
    }
}

fn extract_activation_token(
    options: &std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
) -> Option<String> {
    use std::convert::TryFrom;
    let value = options.get("activation_token")?;
    String::try_from(value.clone()).ok()
}

fn shortcut_to_portal_trigger(shortcut: &str) -> Option<String> {
    let parts: Vec<&str> = shortcut
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let key = parts.last()?;

    let mut trigger_parts = Vec::new();
    for part in &parts[..parts.len().saturating_sub(1)] {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" | "cmdorctrl" | "commandorcontrol" => {
                trigger_parts.push("CTRL".to_string())
            }
            "alt" | "option" => trigger_parts.push("ALT".to_string()),
            "shift" => trigger_parts.push("SHIFT".to_string()),
            "super" | "win" | "meta" => trigger_parts.push("LOGO".to_string()),
            _ => return None,
        }
    }

    trigger_parts.push(normalize_portal_key(key));
    Some(trigger_parts.join("+"))
}

fn normalize_portal_key(key: &str) -> String {
    match key.trim().to_lowercase().as_str() {
        "esc" => "Escape".to_string(),
        "return" | "enter" => "Return".to_string(),
        "space" => "space".to_string(),
        other if other.len() == 1 => other.to_string(),
        _ => key.trim().to_string(),
    }
}
