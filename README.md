# Pot GTK

A native GTK4/libadwaita translation and OCR desktop application for Linux. Built with Rust and relm4, supporting multiple translation backends, screenshot OCR, text-to-speech, system tray, and a JavaScript plugin system.

> This is a native GTK4 rewrite of [pot-desktop](https://github.com/pot-app/pot-desktop), replacing the Tauri/WebView stack with a pure GTK4 + Rust architecture for lower resource usage and better Linux desktop integration.

## Features

### Translation (7 built-in backends)

| Backend | Auth Required | Notes |
|---------|:---:|-------|
| Google Translate | No | Free gtx endpoint, dictionary mode |
| Microsoft Bing | No | Edge auth token (auto-refreshed) |
| DeepL | Optional | Free web scraping / paid API / DeepLX |
| OpenAI / Azure | Yes | Configurable model, prompt, extra args |
| Baidu Translate | Yes | MD5 signature |
| Youdao Translate | Yes | SHA-256 signature, dictionary mode |
| Lingva | No | Self-hosted Google Translate proxy |

All enabled services run in parallel. Results are displayed side by side with copy, TTS, and collect actions per result.

### OCR

- System Tesseract integration (CLI invocation)
- Screenshot region selection with multi-backend fallback chain:
  - GNOME Shell D-Bus (Wayland), grim+slurp, gnome-screenshot, Spectacle, Flameshot, maim, scrot, ImageMagick import
- XDG Desktop Portal screenshot with interactive region selection
- One-click send recognized text to translation

### Text-to-Speech

- Lingva TTS API (built-in)
- GStreamer audio playback (feature-gated)

### Clipboard Monitoring

- Polls clipboard for new text (500ms interval)
- Automatically triggers translation when enabled

### HTTP API Server

Listens on `127.0.0.1:60828` (configurable) for external triggers:

| Endpoint | Method | Action |
|----------|--------|--------|
| `/translate` | POST | Translate request body text |
| `/selection_translate` | GET | Translate selected text |
| `/input_translate` | GET | Open input translate window |
| `/ocr_recognize` | GET | OCR on screenshot selection |
| `/ocr_translate` | GET | OCR + translate |
| `/config` | GET | Open settings window |

### Global Hotkeys (X11 + Wayland)

Automatic backend selection based on session type:

**X11:** Direct key grabbing via x11rb

**Wayland:** Multi-backend with deduplication:
- GNOME custom keybindings (gsettings)
- XDG Desktop Portal global shortcuts (ashpd)
- evdev direct input device listener

| Default Shortcut | Action |
|---|---|
| `Ctrl+Shift+S` | Selection translate |
| `Ctrl+Shift+I` | Input translate |
| `Ctrl+Shift+O` | OCR recognize |
| `Ctrl+Shift+T` | OCR + translate |

All hotkeys are configurable in settings. Wayland backends run in parallel with time-windowed (800ms) duplicate suppression.

### Plugin System

JavaScript plugins packaged as `.potext` archives (ZIP). Uses `boa_engine` as a pure Rust JS runtime. Plugins can add new translation, OCR, TTS, or collection backends without recompiling.

### Other

- **Collection** — Send translation results to Anki via AnkiConnect
- **History** — SQLite-backed translation history with pagination
- **Language Detection** — Local detection for 22 languages via lingua
- **System Tray** — ksni (KDE StatusNotifierItem) with GTK fallback
- **i18n** — 9 languages (de, es, fr, ja, ko, ru, zh_CN, zh_TW + template) via gettext
- **Theme** — Light / Dark / System via libadwaita
- **Autostart** — Auto start on login (XDG autostart)
- **Proxy** — HTTP/HTTPS proxy configuration
- **Update Checker** — GitHub Releases API

## Architecture

### Module Structure

```
src/
├── main.rs                   # Entry point: adw::Application bootstrap
├── app.rs                    # Root relm4 component, message routing hub
├── config.rs                 # JSON-backed config store (XDG_CONFIG_HOME)
├── error.rs                  # Unified AppError enum
├── i18n.rs                   # gettext init + theme application
├── util.rs                   # nanoid() ID generation
│
├── core/                     # Platform infrastructure
│   ├── autostart.rs          # Autostart management (XDG autostart)
│   ├── clipboard.rs          # Clipboard monitoring (arboard)
│   ├── font.rs               # System font enumeration (font-kit)
│   ├── history.rs            # SQLite history store (rusqlite)
│   ├── hotkey.rs             # Global hotkeys (X11 + Wayland, feature-gated)
│   ├── http_server.rs        # Local HTTP API (tiny_http)
│   ├── image_utils.rs        # Screenshot crop + base64
│   ├── lang_detect.rs        # Language detection (lingua, feature-gated)
│   ├── proxy.rs              # HTTP proxy env-var setup
│   ├── runtime.rs            # Shared tokio runtime (OnceLock)
│   ├── screenshot.rs         # Screen capture (X11 + Wayland + portal)
│   └── tray.rs               # System tray (ksni / GTK fallback, feature-gated)
│
├── lang/
│   └── mod.rs                # Language enum (30 variants), ISO code mappings
│
├── services/                 # External service integrations
│   ├── types.rs              # Shared request/result/error types
│   ├── translate/            # Translation backends
│   │   ├── mod.rs            # TranslateService trait + TranslateRegistry
│   │   ├── baidu.rs          # Baidu (MD5 signed)
│   │   ├── bing.rs           # Bing (Edge token, cached)
│   │   ├── deepl.rs          # DeepL (free/API/DeepLX)
│   │   ├── google.rs         # Google (free gtx, dictionary)
│   │   ├── lingva.rs         # Lingva (self-hosted)
│   │   ├── openai.rs         # OpenAI / Azure
│   │   └── youdao.rs         # Youdao (SHA-256 signed, dictionary)
│   ├── recognize/
│   │   └── mod.rs            # RecognizeService trait + SystemTesseract
│   ├── tts/
│   │   └── mod.rs            # TtsService trait + LingvaTts
│   ├── collection/
│   │   └── mod.rs            # CollectionService trait + AnkiCollection
│   └── plugin/
│       └── mod.rs            # PluginManager (.potext archives), boa_engine runtime
│
└── windows/                  # relm4 UI components
    ├── config.rs             # Settings window (General/Translate/Services/Hotkey/About)
    ├── service_config.rs     # Service instance CRUD (add/edit/delete/reorder)
    ├── translate.rs          # Translation window (source, language selectors, results)
    ├── recognize.rs          # OCR window (image preview, recognized text)
    ├── screenshot.rs         # Fullscreen selection overlay (Cairo)
    └── updater.rs            # Update checker dialog
```

### Data Flow

```
User Action (hotkey / clipboard / HTTP API / tray)
        │
        ▼
   app.rs (message hub)
        │
        ├── TranslateText ──► TranslateModel ──► TranslateRegistry ──► backends
        ├── OcrRecognize ───► ScreenshotModel ──► RecognizeModel ──► RecognizeRegistry
        ├── ShowConfig ─────► ConfigModel ──► ServiceConfigModel
        └── ClipboardEvent ► TranslateModel (auto-translate)

Background work runs in relm4 command threads.
Results are sent back to the GTK main loop via CommandOutput.
```

### Service Architecture

All service categories share the same pattern:

```rust
trait XxxService {
    fn name(&self) -> &str;
    fn execute(&self, req: XxxRequest) -> Result<XxxResult>;
}

struct XxxRegistry {
    services: HashMap<String, Box<dyn XxxService>>,
}
```

Registries are created in `app.rs` and passed to both the settings UI (for configuration) and the functional windows (for execution).

### Configuration

Config stored as JSON at `$XDG_CONFIG_HOME/com.pot-app.desktop/config.json`. Compatible with the existing pot-desktop config format. Key entries:

- `translate_service_list` — Ordered list of enabled translation service instances
- `recognize_service_list` — Ordered list of OCR service instances
- `tts_service_list` — TTS service instances
- `collection_service_list` — Collection service instances
- `app_theme` — `light` / `dark` / `system`
- `app_language` — UI language
- `server_port` — HTTP API port (default 60828)
- `hotkey_*` — Keyboard shortcut definitions
- Proxy settings, window preferences, etc.

### Supported Languages

30 source/target languages: Auto, Chinese (Simplified/Traditional), English, Japanese, Korean, French, Spanish, German, Russian, Italian, Portuguese, Turkish, Arabic, Vietnamese, Thai, Indonesian, Malay, Hindi, Mongolian, Norwegian (Nynorsk/Bokmal), Persian, Ukrainian, Polish, Dutch, Romanian, Czech, Hungarian, Greek.

## Build

### Prerequisites

| Dependency | Required | Feature |
|---|:---:|---|
| Rust 1.80+ | Yes | — |
| GTK4 development headers | Yes | — |
| libadwaita 1.5+ | Yes | — |
| GStreamer development headers | No | `tts` |
| X11 development headers | No | `hotkey` |
| gettext (`msgfmt`) | No | locale compilation |
| Tesseract OCR | No | OCR runtime |

Install on Ubuntu/Debian:

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev \
    libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
    libx11-dev gettext
```

### Feature Flags

| Feature | Default | Dependencies | Description |
|---------|:---:|---|---|
| `tts` | Yes | gstreamer, gstreamer-app | Text-to-speech playback |
| `ocr` | Yes | lingua | Language detection for OCR |
| `hotkey` | Yes | x11rb, ashpd, zbus, evdev | Global hotkeys (X11 + Wayland) |
| `plugin` | No | boa_engine | JavaScript plugin runtime |
| `tray` | Yes | ksni, futures | System tray icon |

### Quick Build

```bash
# Default (tts + ocr + hotkey + tray)
cargo build --release

# Minimal (no optional features)
cargo build --release --no-default-features

# All features
cargo build --release --all-features

# Specific features
cargo build --release --no-default-features --features "tts,hotkey"
```

### Interactive Build Script

```bash
./build.sh
```

The script walks through:
1. Feature selection (TTS / OCR / hotkey / plugin / tray)
2. Locale compilation (which languages to include)
3. Output format (binary / .deb / .rpm / AppImage)

### Docker Build

No local Rust or GTK dev headers required:

```bash
# Build binary → output/pot-gtk
docker compose run --rm build

# Build .deb package → output/*.deb
docker compose run --rm deb

# Build .rpm package → output/*.rpm
docker compose run --rm rpm

# Build AppImage → output/*.AppImage
docker compose run --rm appimage

# Interactive development shell
docker compose run --rm shell

# With specific features
FEATURES=tts,hotkey docker compose run --rm build
```

The Docker image uses Ubuntu 24.04 with dependency caching via a named volume (`cargo-cache`).

### Packaging

**deb (Debian/Ubuntu):**

```bash
cargo install cargo-deb
cargo deb
# Output: target/debian/pot-gtk_0.1.0_amd64.deb
```

**rpm (Fedora/RHEL):**

```bash
cargo install cargo-generate-rpm
cargo build --release
cargo generate-rpm
# Output: target/generate-rpm/pot-gtk-0.1.0.x86_64.rpm
```

**AppImage (portable):**

Built via Docker with `linuxdeploy`. Includes binary, desktop file, icons, and compiled locales:

```bash
docker compose run --rm appimage
# Output: output/Pot-GTK-x86_64.AppImage
```

**Flatpak:**

```bash
flatpak-builder build-dir flatpak/com.pot-app.pot-gtk.yml
```

## Development

### Project Conventions

- **UI**: relm4 Elm-style MVC — models hold state, `update()` handles messages, views are declared in `init_view()`
- **Services**: Trait + Registry pattern. New backends implement the trait and register in the factory
- **Config**: `Arc<AppConfig>` shared across threads. All reads/writes go through `get()`/`set()`
- **Errors**: `AppError` with `thiserror`. Use `anyhow` for application-level context
- **Async**: Shared tokio runtime via `core::runtime` (`OnceLock`). Background work in relm4 `spawn_command()`. Never block the GTK main loop

### Adding a Translation Backend

1. Create `src/services/translate/<name>.rs`
2. Implement `TranslateService`:

```rust
pub struct MyTranslator;

impl TranslateService for MyTranslator {
    fn name(&self) -> &str { "my_translator" }

    fn translate(&self, req: TranslateRequest) -> Result<TranslateResult> {
        let resp = http_client()
            .post("https://api.example.com/translate")
            .json(&serde_json::json!({ "text": req.text }))
            .send()?;
        // parse response...
        Ok(TranslateResult::Text(translated))
    }

    fn default_config(&self) -> serde_json::Value {
        serde_json::json!({ "api_key": "" })
    }
}
```

3. Register in `services/translate/mod.rs` → `create_registry()`
4. Add config fields in `windows/service_config.rs` → `build_fields()`
5. Add the service name to the built-in list in `config.rs`

### Adding a Plugin (.potext)

A plugin is a ZIP archive containing:

```
my-plugin.potext
├── info.json          # name, plugin_type, display, language
└── main.js            # Entry point
```

`info.json`:

```json
{
  "name": "my-translator",
  "plugin_type": "translate",
  "display": { "en": "My Translator", "zh_CN": "我的翻译" },
  "language": { "source": ["en"], "target": ["zh-CN", "ja"] }
}
```

`main.js` receives input via a global variable and must return a string:

```javascript
function translate(text, from, to, config) {
    // Call API, return translated text
    return "translated text";
}
translate(translate_text, translate_from, translate_to, translate_config);
```

Install via the settings UI or by placing the `.potext` file in `$XDG_CONFIG_HOME/com.pot-app.desktop/plugins/translate/`.

### Running Tests

```bash
cargo test
cargo test -- --nocapture     # Show println output
cargo test test_name          # Run specific test
```

### Code Quality

```bash
cargo fmt                     # Format
cargo clippy -- -D warnings   # Lint
```

### Locale Updates

```bash
# Extract new translatable strings
xgettext -o data/po/pot-gtk.pot src/**/*.rs

# Update a specific language
msgmerge data/po/zh_CN.po data/po/pot-gtk.pot -U

# Compile (also done by build.rs / build.sh)
msgfmt data/po/zh_CN.po -o data/po/zh_CN/LC_MESSAGES/pot-gtk.mo
```

## Runtime Dependencies

| Library | Required | Purpose |
|---------|:---:|---|
| `libgtk-4` | Yes | UI framework |
| `libadwaita-1` | Yes | Adwaita widgets |
| `libgstreamer-1.0` | No | TTS audio playback |
| `libX11` | No | X11 global hotkeys |
| `tesseract-ocr` | No | OCR engine |

## License

GPL-3.0-only
