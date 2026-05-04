use crate::config::APP_ID;
use crate::error::{AppError, Result};
use log::{info, warn};
use screenshots::Screen;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[cfg(feature = "hotkey")]
const PORTAL_APP_ID: &str = "com.pot_app.pot_gtk";

#[derive(Debug, Clone)]
pub struct CaptureInfo {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

fn cache_dir() -> Result<PathBuf> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| AppError::Custom("Failed to get cache directory".into()))?;
    let dir = dir.join(APP_ID);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn screenshot_save_path() -> Result<PathBuf> {
    Ok(cache_dir()?.join("pot_screenshot.png"))
}

fn screenshot_cut_path() -> Result<PathBuf> {
    Ok(cache_dir()?.join("pot_screenshot_cut.png"))
}

/// Capture the screen containing the mouse cursor.
/// On Wayland, uses the XDG Screenshot portal to avoid black-screen issues.
/// On X11, uses the `screenshots` crate with the correct monitor.
pub fn capture_all_screens() -> Result<CaptureInfo> {
    let session_type = detect_session_type();
    match session_type.as_str() {
        "wayland" => capture_via_portal().or_else(|e| {
            warn!("Portal screenshot failed ({}), falling back to XCB", e);
            capture_via_xcb()
        }),
        _ => capture_via_xcb(),
    }
}

pub fn is_wayland_session() -> bool {
    detect_session_type() == "wayland"
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

/// Ask the desktop environment to perform an interactive region selection.
/// Native tools handle compositor permissions and multi-monitor coordinates
/// better than a client-side GTK overlay.
pub fn capture_interactive_selection() -> Result<CaptureInfo> {
    capture_via_native_region_tools().or_else(|e| {
        warn!("Native screenshot region tools failed: {}", e);
        capture_via_interactive_portal()
    })
}

#[cfg(feature = "hotkey")]
fn capture_via_interactive_portal() -> Result<CaptureInfo> {
    capture_via_portal_with_interaction(true, true)
}

#[cfg(not(feature = "hotkey"))]
fn capture_via_interactive_portal() -> Result<CaptureInfo> {
    Err(AppError::Custom(
        "XDG screenshot portal not available (hotkey feature disabled)".into(),
    ))
}

fn capture_via_xcb() -> Result<CaptureInfo> {
    let screens = Screen::all().map_err(|e| AppError::Custom(e.to_string()))?;
    let save_path = screenshot_save_path()?;

    let (mx, my) = match mouse_position::mouse_position::Mouse::get_mouse_position() {
        mouse_position::mouse_position::Mouse::Position { x, y } => (x as i32, y as i32),
        _ => (0, 0),
    };

    info!("Capturing screen at mouse position: ({}, {})", mx, my);

    for screen in &screens {
        let info = screen.display_info;
        if mx >= info.x
            && mx < info.x + info.width as i32
            && my >= info.y
            && my < info.y + info.height as i32
        {
            let image = screen
                .capture()
                .map_err(|e| AppError::Custom(format!("Screen capture failed: {}", e)))?;
            let buffer = image
                .to_png(screenshots::Compression::Fast)
                .map_err(|e| AppError::Custom(format!("PNG encoding failed: {}", e)))?;
            let width = image.width();
            let height = image.height();
            fs::write(&save_path, buffer)?;
            info!("Screenshot saved to {:?}", save_path);
            return Ok(CaptureInfo {
                path: save_path,
                width,
                height,
            });
        }
    }

    // Fallback: capture first screen
    if let Some(screen) = screens.first() {
        let image = screen
            .capture()
            .map_err(|e| AppError::Custom(format!("Screen capture failed: {}", e)))?;
        let buffer = image
            .to_png(screenshots::Compression::Fast)
            .map_err(|e| AppError::Custom(format!("PNG encoding failed: {}", e)))?;
        let width = image.width();
        let height = image.height();
        fs::write(&save_path, buffer)?;
        info!("Screenshot saved (fallback) to {:?}", save_path);
        return Ok(CaptureInfo {
            path: save_path,
            width,
            height,
        });
    }

    Err(AppError::Custom("No screens found".into()))
}

fn capture_info_for_file(path: PathBuf) -> Result<CaptureInfo> {
    let (width, height) = image::image_dimensions(&path)
        .map_err(|e| AppError::Custom(format!("Failed to read screenshot image size: {}", e)))?;
    Ok(CaptureInfo {
        path,
        width,
        height,
    })
}

fn capture_via_native_region_tools() -> Result<CaptureInfo> {
    let mut errors = Vec::new();
    for tool in preferred_region_tools() {
        let result = match tool {
            RegionTool::GnomeShell => capture_via_gnome_shell(),
            RegionTool::GrimSlurp => capture_via_grim_slurp(),
            RegionTool::GnomeScreenshot => capture_via_gnome_screenshot(),
            RegionTool::Spectacle => capture_via_spectacle(),
            RegionTool::Flameshot => capture_via_flameshot(),
            RegionTool::Maim => capture_via_maim(),
            RegionTool::Scrot => capture_via_scrot(),
            RegionTool::ImageMagickImport => capture_via_imagemagick_import(),
        };

        match result {
            Ok(capture) => return Ok(capture),
            Err(e) => {
                let message = format!("{}: {}", tool.label(), e);
                warn!("Native screenshot region capture failed via {}", message);
                errors.push(message);
            }
        }
    }

    if errors.is_empty() {
        Err(AppError::Custom(
            "No supported native region screenshot tool found in PATH".into(),
        ))
    } else {
        Err(AppError::Custom(format!(
            "No native screenshot region tool succeeded ({})",
            errors.join(" | ")
        )))
    }
}

#[derive(Clone, Copy)]
enum RegionTool {
    GnomeShell,
    GrimSlurp,
    GnomeScreenshot,
    Spectacle,
    Flameshot,
    Maim,
    Scrot,
    ImageMagickImport,
}

impl RegionTool {
    fn label(self) -> &'static str {
        match self {
            RegionTool::GnomeShell => "gnome-shell",
            RegionTool::GrimSlurp => "grim/slurp",
            RegionTool::GnomeScreenshot => "gnome-screenshot",
            RegionTool::Spectacle => "spectacle",
            RegionTool::Flameshot => "flameshot",
            RegionTool::Maim => "maim",
            RegionTool::Scrot => "scrot",
            RegionTool::ImageMagickImport => "import",
        }
    }
}

fn preferred_region_tools() -> Vec<RegionTool> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    let session = detect_session_type();
    let mut tools = Vec::new();

    if desktop.contains("gnome") {
        tools.push(RegionTool::GnomeShell);
    }
    if session == "wayland" && command_exists("grim") && command_exists("slurp") {
        tools.push(RegionTool::GrimSlurp);
    }
    if desktop.contains("gnome") && command_exists("gnome-screenshot") {
        tools.push(RegionTool::GnomeScreenshot);
    }
    if (desktop.contains("kde") || desktop.contains("plasma")) && command_exists("spectacle") {
        tools.push(RegionTool::Spectacle);
    }
    if command_exists("flameshot") {
        tools.push(RegionTool::Flameshot);
    }
    if session == "x11" && command_exists("maim") {
        tools.push(RegionTool::Maim);
    }
    if session == "x11" && command_exists("scrot") {
        tools.push(RegionTool::Scrot);
    }
    if session == "x11" && command_exists("import") {
        tools.push(RegionTool::ImageMagickImport);
    }

    let mut fallbacks = vec![
        (RegionTool::GnomeScreenshot, "gnome-screenshot"),
        (RegionTool::Spectacle, "spectacle"),
        (RegionTool::Flameshot, "flameshot"),
    ];
    if session == "x11" {
        fallbacks.extend([
            (RegionTool::Maim, "maim"),
            (RegionTool::Scrot, "scrot"),
            (RegionTool::ImageMagickImport, "import"),
        ]);
    }
    for (tool, command) in fallbacks {
        if command_exists(command)
            && !tools
                .iter()
                .any(|existing| existing.label() == tool.label())
        {
            tools.push(tool);
        }
    }

    tools
}

fn command_exists(command: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };

    std::env::split_paths(&paths).any(|dir| {
        let candidate = dir.join(command);
        candidate.is_file()
    })
}

#[cfg(feature = "hotkey")]
fn capture_via_gnome_shell() -> Result<CaptureInfo> {
    let cut_path = screenshot_cut_path()?;
    remove_file_if_exists(&cut_path)?;
    let filename = cut_path.to_string_lossy().to_string();

    let conn = zbus::blocking::Connection::session()
        .map_err(|e| AppError::Custom(format!("GNOME Shell screenshot bus: {}", e)))?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.gnome.Shell.Screenshot",
        "/org/gnome/Shell/Screenshot",
        "org.gnome.Shell.Screenshot",
    )
    .map_err(|e| AppError::Custom(format!("GNOME Shell screenshot proxy: {}", e)))?;

    let (x, y, width, height): (i32, i32, i32, i32) = proxy
        .call("SelectArea", &())
        .map_err(|e| AppError::Custom(format!("GNOME Shell SelectArea failed: {}", e)))?;
    if width <= 0 || height <= 0 {
        return Err(AppError::Custom("GNOME Shell selection is empty".into()));
    }

    let (success, filename_used): (bool, String) = proxy
        .call(
            "ScreenshotArea",
            &(x, y, width, height, false, filename.as_str()),
        )
        .map_err(|e| AppError::Custom(format!("GNOME Shell ScreenshotArea failed: {}", e)))?;
    if !success {
        return Err(AppError::Custom(
            "GNOME Shell ScreenshotArea reported failure".into(),
        ));
    }

    let final_path = if filename_used.is_empty() {
        cut_path
    } else {
        PathBuf::from(filename_used)
    };
    finish_region_capture(final_path, "gnome-shell")
}

#[cfg(not(feature = "hotkey"))]
fn capture_via_gnome_shell() -> Result<CaptureInfo> {
    Err(AppError::Custom(
        "GNOME Shell screenshot backend not available (hotkey feature disabled)".into(),
    ))
}

fn capture_via_grim_slurp() -> Result<CaptureInfo> {
    let slurp = Command::new("slurp")
        .output()
        .map_err(|e| AppError::Custom(format!("failed to run slurp: {}", e)))?;
    if !slurp.status.success() {
        return Err(AppError::Custom(format!(
            "slurp exited with status {:?}: {}",
            slurp.status.code(),
            String::from_utf8_lossy(&slurp.stderr)
        )));
    }

    let geometry = String::from_utf8_lossy(&slurp.stdout).trim().to_string();
    if geometry.is_empty() {
        return Err(AppError::Custom("slurp returned empty geometry".into()));
    }

    let cut_path = screenshot_cut_path()?;
    remove_file_if_exists(&cut_path)?;
    let output = Command::new("grim")
        .arg("-g")
        .arg(&geometry)
        .arg(&cut_path)
        .output()
        .map_err(|e| AppError::Custom(format!("failed to run grim: {}", e)))?;
    if !output.status.success() {
        return Err(AppError::Custom(format!(
            "grim exited with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    finish_region_capture(cut_path, "grim/slurp")
}

fn capture_via_gnome_screenshot() -> Result<CaptureInfo> {
    let cut_path = screenshot_cut_path()?;
    remove_file_if_exists(&cut_path)?;
    let output = Command::new("gnome-screenshot")
        .arg("-a")
        .arg("-f")
        .arg(&cut_path)
        .output()
        .map_err(|e| AppError::Custom(format!("failed to run gnome-screenshot: {}", e)))?;
    if !output.status.success() {
        return Err(AppError::Custom(format!(
            "gnome-screenshot exited with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    finish_region_capture(cut_path, "gnome-screenshot")
}

fn capture_via_spectacle() -> Result<CaptureInfo> {
    let cut_path = screenshot_cut_path()?;
    remove_file_if_exists(&cut_path)?;
    let output = Command::new("spectacle")
        .arg("-b")
        .arg("-r")
        .arg("-o")
        .arg(&cut_path)
        .output()
        .map_err(|e| AppError::Custom(format!("failed to run spectacle: {}", e)))?;
    if !output.status.success() {
        return Err(AppError::Custom(format!(
            "spectacle exited with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    finish_region_capture(cut_path, "spectacle")
}

fn capture_via_flameshot() -> Result<CaptureInfo> {
    let cut_path = screenshot_cut_path()?;
    remove_file_if_exists(&cut_path)?;
    let output = Command::new("flameshot")
        .arg("gui")
        .arg("--raw")
        .output()
        .map_err(|e| AppError::Custom(format!("failed to run flameshot: {}", e)))?;
    if !output.status.success() {
        return Err(AppError::Custom(format!(
            "flameshot exited with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    if output.stdout.is_empty() {
        return Err(AppError::Custom(
            "flameshot returned empty image data".into(),
        ));
    }

    fs::write(&cut_path, output.stdout)?;
    finish_region_capture(cut_path, "flameshot")
}

fn capture_via_maim() -> Result<CaptureInfo> {
    let cut_path = screenshot_cut_path()?;
    remove_file_if_exists(&cut_path)?;
    let output = Command::new("maim")
        .arg("-s")
        .arg(&cut_path)
        .output()
        .map_err(|e| AppError::Custom(format!("failed to run maim: {}", e)))?;
    if !output.status.success() {
        return Err(AppError::Custom(format!(
            "maim exited with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    finish_region_capture(cut_path, "maim")
}

fn capture_via_scrot() -> Result<CaptureInfo> {
    let cut_path = screenshot_cut_path()?;
    remove_file_if_exists(&cut_path)?;
    let output = Command::new("scrot")
        .arg("-s")
        .arg(&cut_path)
        .output()
        .map_err(|e| AppError::Custom(format!("failed to run scrot: {}", e)))?;
    if !output.status.success() {
        return Err(AppError::Custom(format!(
            "scrot exited with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    finish_region_capture(cut_path, "scrot")
}

fn capture_via_imagemagick_import() -> Result<CaptureInfo> {
    let cut_path = screenshot_cut_path()?;
    remove_file_if_exists(&cut_path)?;
    let output = Command::new("import")
        .arg(&cut_path)
        .output()
        .map_err(|e| AppError::Custom(format!("failed to run import: {}", e)))?;
    if !output.status.success() {
        return Err(AppError::Custom(format!(
            "import exited with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    finish_region_capture(cut_path, "import")
}

fn finish_region_capture(cut_path: PathBuf, tool: &str) -> Result<CaptureInfo> {
    let metadata = fs::metadata(&cut_path).map_err(|e| {
        AppError::Custom(format!(
            "{} did not create a readable screenshot file: {}",
            tool, e
        ))
    })?;
    if metadata.len() == 0 {
        return Err(AppError::Custom(format!(
            "{} created an empty screenshot file",
            tool
        )));
    }

    fs::copy(&cut_path, screenshot_save_path()?)?;
    info!(
        "Interactive screenshot saved via {} to {:?}",
        tool, cut_path
    );
    capture_info_for_file(cut_path)
}

fn remove_file_if_exists(path: &PathBuf) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(AppError::Custom(format!(
            "Failed to remove stale screenshot {:?}: {}",
            path, e
        ))),
    }
}

#[cfg(feature = "hotkey")]
fn capture_via_portal() -> Result<CaptureInfo> {
    capture_via_portal_with_interaction(false, false)
}

#[cfg(feature = "hotkey")]
fn capture_via_portal_with_interaction(
    interactive: bool,
    save_as_cut: bool,
) -> Result<CaptureInfo> {
    use ashpd::desktop::screenshot::Screenshot;

    let handle = crate::core::runtime::handle()
        .ok_or_else(|| AppError::Custom("shared tokio runtime not initialized".into()))?;

    handle.block_on(async {
        register_portal_host_app().await;

        let response = Screenshot::request()
            .interactive(interactive)
            .send()
            .await
            .map_err(|e| AppError::Custom(format!("Screenshot portal request failed: {}", e)))?
            .response()
            .map_err(|e| AppError::Custom(format!("Screenshot portal response: {}", e)))?;

        let uri = response.uri().to_string();
        info!("Portal screenshot URI: {}", uri);

        // Read the portal URI — try file path first, then gio fallback
        let png_data = if let Ok(path) = response.uri().to_file_path() {
            std::fs::read(&path).map_err(|e| {
                AppError::Custom(format!("Failed to read portal screenshot file: {}", e))
            })?
        } else {
            // Fallback: use gio to read the URI
            use gtk4::prelude::FileExt;
            let file = gtk4::gio::File::for_uri(&uri);
            let (bytes, _etag): (gtk4::glib::Bytes, Option<gtk4::glib::GString>) =
                file.load_bytes(gtk4::gio::Cancellable::NONE).map_err(|e| {
                    AppError::Custom(format!("Failed to read portal screenshot: {}", e))
                })?;
            bytes.to_vec()
        };

        let save_path = screenshot_save_path()?;
        fs::write(&save_path, &png_data)?;
        let final_path = if save_as_cut {
            let cut_path = screenshot_cut_path()?;
            fs::write(&cut_path, &png_data)?;
            cut_path
        } else {
            save_path
        };

        let (width, height) = image::image_dimensions(&final_path)
            .map_err(|e| AppError::Custom(format!("Failed to read portal image size: {}", e)))?;
        info!("Portal screenshot saved to {:?}", final_path);
        Ok(CaptureInfo {
            path: final_path,
            width,
            height,
        })
    })
}

#[cfg(feature = "hotkey")]
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

#[cfg(not(feature = "hotkey"))]
fn capture_via_portal() -> Result<CaptureInfo> {
    Err(AppError::Custom(
        "Portal screenshot not available (hotkey feature disabled)".into(),
    ))
}
