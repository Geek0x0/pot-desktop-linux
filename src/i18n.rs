use gettextrs::{
    bind_textdomain_codeset, bindtextdomain, gettext, setlocale, textdomain, LocaleCategory,
};
use std::path::PathBuf;

const TEXT_DOMAIN: &str = "pot-gtk";

pub fn init() {
    let locale_dir = locale_dir();

    // Honor LANGUAGE env var (set by main() from config before this is called).
    // Use empty string to pick up env-driven locale; fall back to C.UTF-8.
    setlocale(LocaleCategory::LcAll, "")
        .or_else(|| setlocale(LocaleCategory::LcAll, "C.UTF-8"))
        .or_else(|| setlocale(LocaleCategory::LcAll, "C"));

    let _ = bindtextdomain(TEXT_DOMAIN, &locale_dir);
    let _ = bind_textdomain_codeset(TEXT_DOMAIN, "UTF-8");
    let _ = textdomain(TEXT_DOMAIN);

    log::info!(
        "i18n initialized, locale dir: {}, LANGUAGE={:?}",
        locale_dir.display(),
        std::env::var("LANGUAGE").ok()
    );
}

/// Map app-internal language code (used in config) to gettext locale name
/// (the directory name under <locale_dir>/<code>/LC_MESSAGES/).
fn to_gettext_locale(lang: &str) -> &str {
    match lang {
        "zh_cn" => "zh_CN",
        "zh_tw" => "zh_TW",
        "pt_br" => "pt_BR",
        other => other,
    }
}

pub fn set_language(lang: &str) {
    // LANGUAGE is the standard gettext override and works without the
    // system locale being installed. Must be set BEFORE bindtextdomain/
    // textdomain are called for first init; for runtime changes, takes
    // effect after restart.
    let locale = to_gettext_locale(lang);
    std::env::set_var("LANGUAGE", locale);
    refresh_locale_from_env();
    log::info!("Language set to: {} (LANGUAGE={})", lang, locale);
}

pub fn t(msgid: &str) -> String {
    gettext(msgid)
}

fn refresh_locale_from_env() {
    let _ = setlocale(LocaleCategory::LcAll, "")
        .or_else(|| setlocale(LocaleCategory::LcAll, "C.UTF-8"))
        .or_else(|| setlocale(LocaleCategory::LcAll, "C"));
    let _ = textdomain(TEXT_DOMAIN);
}

pub fn apply_theme(theme: &str) {
    let style_manager = relm4::adw::StyleManager::default();
    let scheme = match theme {
        "light" => relm4::adw::ColorScheme::ForceLight,
        "dark" => relm4::adw::ColorScheme::ForceDark,
        _ => relm4::adw::ColorScheme::Default,
    };
    style_manager.set_color_scheme(scheme);
}

fn locale_dir() -> PathBuf {
    // 1. Check next to the binary (output/locales/ or installed prefix)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let locale_dir = dir.join("locales");
            if locale_dir.join("zh_CN/LC_MESSAGES/pot-gtk.mo").exists() {
                return locale_dir;
            }
            // Also check ../share/locale relative to bin (standard install layout)
            if let Some(prefix) = dir.parent() {
                let share_locale = prefix.join("share").join("locale");
                if share_locale.join("zh_CN/LC_MESSAGES/pot-gtk.mo").exists() {
                    return share_locale;
                }
            }
        }
    }

    // 2. Check build directory (CARGO_MANIFEST_DIR)
    let dev_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("po");
    if dev_dir
        .join("zh_CN")
        .join("LC_MESSAGES")
        .join("pot-gtk.mo")
        .exists()
    {
        return dev_dir;
    }

    // 3. Fallback to system locale path
    PathBuf::from("/usr/share/locale")
}
