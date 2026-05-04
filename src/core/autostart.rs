use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

const DESKTOP_FILE_NAME: &str = "com.pot-app.pot-gtk.desktop";

const AUTOSTART_DESKTOP_ENTRY: &str = "\
[Desktop Entry]
Type=Application
Name=Pot GTK
Exec=pot-gtk
Icon=com.pot-app.pot-gtk
Terminal=false
X-GNOME-Autostart-enabled=true
Hidden=false
";

fn autostart_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("autostart"))
}

pub fn is_enabled() -> bool {
    autostart_dir()
        .map(|dir| dir.join(DESKTOP_FILE_NAME).exists())
        .unwrap_or(false)
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let dir = autostart_dir().context("cannot determine autostart directory")?;
    let path = dir.join(DESKTOP_FILE_NAME);

    if enabled {
        fs::create_dir_all(&dir).with_context(|| format!("cannot create {:?}", dir))?;
        fs::write(&path, AUTOSTART_DESKTOP_ENTRY)
            .with_context(|| format!("cannot write {:?}", path))?;
        log::info!("Autostart enabled: {:?}", path);
    } else if path.exists() {
        fs::remove_file(&path).with_context(|| format!("cannot remove {:?}", path))?;
        log::info!("Autostart disabled: removed {:?}", path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_entry_is_valid() {
        assert!(AUTOSTART_DESKTOP_ENTRY.contains("X-GNOME-Autostart-enabled=true"));
        assert!(AUTOSTART_DESKTOP_ENTRY.contains("Hidden=false"));
        assert!(AUTOSTART_DESKTOP_ENTRY.contains("Type=Application"));
        assert!(AUTOSTART_DESKTOP_ENTRY.contains("Exec=pot-gtk"));
    }
}
