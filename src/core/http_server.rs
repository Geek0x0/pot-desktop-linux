use log::{info, warn};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tiny_http::{Response, Server};
use tokio::sync::mpsc;

const MAX_BODY_SIZE: u64 = 1024 * 1024; // 1MB
const RATE_LIMIT_WINDOW_SECS: u64 = 60;
const RATE_LIMIT_MAX_REQUESTS: u32 = 120;

#[derive(Debug, Clone)]
pub enum AppAction {
    TranslateText(String),
    InputTranslate,
    SelectionTranslate,
    OcrRecognize {
        screenshot: bool,
    },
    OcrTranslate {
        screenshot: bool,
    },
    ShowConfig,
    #[allow(dead_code)]
    ShowUpdater,
    #[allow(dead_code)]
    Quit,
}

/// Activation context from the input source that triggered an action.
/// Used for window placement hints (Wayland startup tokens, X11 timestamps).
#[derive(Debug, Clone, Default)]
pub struct ActivationContext {
    /// XDG activation token from GlobalShortcuts portal (Wayland).
    pub startup_id: Option<String>,
    /// Timestamp from the input event (milliseconds), for present_with_time().
    pub timestamp: Option<u64>,
}

/// An AppAction paired with optional activation context from the trigger source.
#[derive(Debug, Clone)]
pub struct ActionEvent {
    pub action: AppAction,
    pub ctx: ActivationContext,
}

impl ActionEvent {
    pub fn new(action: AppAction) -> Self {
        Self {
            action,
            ctx: ActivationContext::default(),
        }
    }

    #[allow(dead_code)]
    pub fn with_startup_id(mut self, token: impl Into<String>) -> Self {
        self.ctx.startup_id = Some(token.into());
        self
    }

    #[allow(dead_code)]
    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.ctx.timestamp = Some(ts);
        self
    }
}

struct RateLimiter {
    window_start: Instant,
    count: u32,
}

impl RateLimiter {
    fn new() -> Self {
        RateLimiter {
            window_start: Instant::now(),
            count: 0,
        }
    }

    fn check(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.window_start).as_secs() >= RATE_LIMIT_WINDOW_SECS {
            self.window_start = now;
            self.count = 0;
        }
        self.count += 1;
        self.count <= RATE_LIMIT_MAX_REQUESTS
    }
}

/// Get or generate the server auth token. If `server_token` is set in config,
/// use that. Otherwise generate a random token and log it so the user can
/// discover it.
fn resolve_token(config: &crate::config::AppConfig) -> String {
    if let Some(value) = config.get("server_token") {
        if let Some(token) = value.as_str() {
            if !token.is_empty() {
                return token.to_string();
            }
        }
    }

    let token = crate::util::nanoid(32);
    let masked = if token.len() > 8 {
        format!("{}****{}", &token[..4], &token[token.len() - 4..])
    } else {
        "****".to_string()
    };
    info!(
        "Generated HTTP server token (set 'server_token' in config to customize): {}",
        masked
    );
    config.set("server_token", &token);
    token
}

pub fn start_server(
    port: i64,
    action_tx: mpsc::UnboundedSender<ActionEvent>,
    config: std::sync::Arc<crate::config::AppConfig>,
) -> thread::JoinHandle<()> {
    let token = resolve_token(&config);
    let rate_limiter = Mutex::new(RateLimiter::new());

    thread::spawn(move || {
        let server = match Server::http(format!("127.0.0.1:{port}")) {
            Ok(s) => s,
            Err(e) => {
                warn!("Server start failed: {}. Change port and restart.", e);
                return;
            }
        };
        info!("HTTP server listening on 127.0.0.1:{}", port);

        for request in server.incoming_requests() {
            // Rate limiting
            {
                let mut limiter = rate_limiter.lock().unwrap_or_else(|e| e.into_inner());
                if !limiter.check() {
                    let _ = request.respond(
                        Response::from_string("rate limit exceeded").with_status_code(429),
                    );
                    continue;
                }
            }

            handle_request(request, &action_tx, &token);
        }
    })
}

fn validate_token(request: &tiny_http::Request, expected: &str) -> bool {
    // Only accept Authorization: Bearer <token> header.
    // Query parameter auth is intentionally not supported to prevent
    // token exposure in logs, browser history, and process listings.
    if let Some(auth_header) = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Authorization"))
    {
        let value = auth_header.value.to_string();
        if value.strip_prefix("Bearer ") == Some(expected) {
            return true;
        }
    }

    false
}

fn handle_request(
    request: tiny_http::Request,
    tx: &mpsc::UnboundedSender<ActionEvent>,
    token: &str,
) {
    let url = request.url().to_string();

    // Health check endpoint — no auth required
    if url == "/" {
        info!("Handle health check");
        respond_ok(request);
        return;
    }

    // All other endpoints require auth
    if !validate_token(&request, token) {
        info!("Rejected unauthenticated request: {}", url);
        let _ = request.respond(Response::from_string("unauthorized").with_status_code(401));
        return;
    }

    info!("Handle {} request", url);

    // Parse path without query parameters
    let path = url.split('?').next().unwrap_or("");
    let query = url.split('?').nth(1).unwrap_or("");

    match path {
        "/translate" => handle_translate(request, tx),
        "/config" => {
            let _ = tx.send(ActionEvent::new(AppAction::ShowConfig));
            respond_ok(request);
        }
        "/selection_translate" => {
            let _ = tx.send(ActionEvent::new(AppAction::SelectionTranslate));
            respond_ok(request);
        }
        "/input_translate" => {
            let _ = tx.send(ActionEvent::new(AppAction::InputTranslate));
            respond_ok(request);
        }
        "/ocr_recognize" => {
            let screenshot = query.contains("screenshot=true") || !query.contains("screenshot=");
            let _ = tx.send(ActionEvent::new(AppAction::OcrRecognize { screenshot }));
            respond_ok(request);
        }
        "/ocr_translate" => {
            let screenshot = query.contains("screenshot=true") || !query.contains("screenshot=");
            let _ = tx.send(ActionEvent::new(AppAction::OcrTranslate { screenshot }));
            respond_ok(request);
        }
        _ => {
            warn!("Unknown request url: {}", url);
            let _ = request.respond(Response::from_string("not found").with_status_code(404));
        }
    }
}

fn handle_translate(mut request: tiny_http::Request, tx: &mpsc::UnboundedSender<ActionEvent>) {
    let mut content = String::new();
    if std::io::Read::take(request.as_reader(), MAX_BODY_SIZE)
        .read_to_string(&mut content)
        .is_ok()
        && !content.is_empty()
    {
        let _ = tx.send(ActionEvent::new(AppAction::TranslateText(content)));
    }
    respond_ok(request);
}

fn respond_ok(request: tiny_http::Request) {
    let _ = request.respond(Response::from_string("ok"));
}

pub fn send_local_action(
    config: &crate::config::AppConfig,
    action: &str,
) -> crate::error::Result<()> {
    let path = match action {
        "selection-translate" | "selection_translate" => "/selection_translate",
        "input-translate" | "input_translate" => "/input_translate",
        "ocr-recognize" | "ocr_recognize" => "/ocr_recognize?screenshot=true",
        "ocr-translate" | "ocr_translate" => "/ocr_translate?screenshot=true",
        "config" | "show-config" | "show_config" => "/config",
        _ => {
            return Err(crate::error::AppError::Custom(format!(
                "Unknown local action '{}'",
                action
            )))
        }
    };

    let token = config
        .get("server_token")
        .and_then(|v| v.as_str().map(String::from))
        .filter(|v| !v.is_empty())
        .ok_or_else(|| crate::error::AppError::Custom("server_token is not configured".into()))?;
    let port = config
        .get("server_port")
        .and_then(|v| v.as_i64())
        .unwrap_or(60828);

    let mut stream = TcpStream::connect(("127.0.0.1", port as u16))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes())?;

    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    if response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200") {
        Ok(())
    } else {
        let status = response.lines().next().unwrap_or("empty response");
        Err(crate::error::AppError::Custom(format!(
            "Local action request failed: {}",
            status
        )))
    }
}
