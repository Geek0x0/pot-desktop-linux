# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.1] - 2026-05-04

### Added

- Auto start on login — toggle in General settings, manages `~/.config/autostart/com.pot-app.pot-gtk.desktop`
- Fixed deb build failure when locale `.mo` files are in subdirectories (glob `*` → `**/*`)

## [1.0.0] - 2026-05-04

### Added

#### Translation
- 7 built-in translation backends: Google, Bing, DeepL, OpenAI/Azure, Baidu, Youdao, Lingva
- Parallel multi-service translation with side-by-side results
- Copy, TTS, and collect actions per result
- Configurable per-service instances with add/edit/delete/reorder

#### OCR
- System Tesseract integration via CLI
- Screenshot region selection with multi-backend fallback chain:
  - GNOME Shell D-Bus (Wayland native, highest priority)
  - grim + slurp (wlroots compositors)
  - gnome-screenshot, Spectacle (KDE), Flameshot, maim, scrot, ImageMagick import
- XDG Desktop Portal screenshot with interactive region selection
- Desktop-aware tool selection (GNOME, KDE, wlroots, X11)
- One-click send recognized text to translation

#### Text-to-Speech
- Lingva TTS API (built-in, no auth required)
- GStreamer audio playback (feature-gated)

#### Global Hotkeys
- X11 backend: direct key grabbing via x11rb
- Wayland backend with three parallel sub-backends:
  - GNOME custom keybindings via gsettings
  - XDG Desktop Portal global shortcuts via ashpd
  - evdev direct input device listener (low-level fallback)
- Automatic session type detection (XDG_SESSION_TYPE, WAYLAND_DISPLAY, DISPLAY)
- Time-windowed (800ms) duplicate suppression for Wayland multi-backend events
- Configurable shortcuts: selection translate, input translate, OCR recognize, OCR + translate

#### Clipboard
- Clipboard polling (500ms interval) with automatic translation trigger

#### HTTP API Server
- Local HTTP server on `127.0.0.1:60828` (configurable port)
- Endpoints: `/translate`, `/selection_translate`, `/input_translate`, `/ocr_recognize`, `/ocr_translate`, `/config`
- `--pot-action` CLI flag for external action triggering via local HTTP

#### Plugin System
- JavaScript plugins as `.potext` archives (ZIP format)
- boa_engine pure Rust JS runtime
- Support for translate, OCR, TTS, and collection plugin types
- Plugin install via settings UI or manual file placement

#### Other Features
- Anki collection via AnkiConnect
- SQLite-backed translation history with pagination
- Local language detection for 22 languages (lingua)
- System tray via ksni (KDE StatusNotifierItem) with GTK fallback
- i18n support for 9 locales (de, es, fr, ja, ko, ru, zh_CN, zh_TW + template) via gettext
- Light / Dark / System theme via libadwaita
- HTTP/HTTPS proxy configuration
- GitHub Releases update checker
- Config compatibility with existing pot-desktop format

#### Build & Packaging
- Docker multi-stage build with feature flags and dependency caching
- Interactive `build.sh` script with feature/locale/package selection
- Package formats:
  - `.deb` (Debian/Ubuntu) via cargo-deb
  - `.rpm` (Fedora/RHEL) via cargo-generate-rpm
  - `.AppImage` (portable) via linuxdeploy
  - Flatpak manifest
- Feature flags: `tts`, `ocr`, `hotkey`, `plugin`, `tray`

### Technical Details
- Written in Rust with GTK4/libadwaita via relm4 (Elm-style MVC)
- Shared tokio runtime via `OnceLock` for async operations
- Trait + Registry service architecture for all backend types
- `Arc<AppConfig>` shared across threads for thread-safe config access
- XDG-compliant config/data/cache directory layout
