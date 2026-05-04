use arboard::Clipboard;
use log::warn;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum ClipboardEvent {
    #[allow(dead_code)]
    NewText(String),
}

fn unlock<T>(guard: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    guard.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct ClipboardMonitor {
    #[allow(dead_code)]
    last_text: Arc<Mutex<String>>,
    enabled: Arc<Mutex<bool>>,
}

impl ClipboardMonitor {
    pub fn new() -> Self {
        Self {
            last_text: Arc::new(Mutex::new(String::new())),
            enabled: Arc::new(Mutex::new(false)),
        }
    }

    #[allow(dead_code)]
    pub fn set_enabled(&self, enabled: bool) {
        *unlock(&self.enabled) = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        *unlock(&self.enabled)
    }

    #[allow(dead_code)]
    pub fn start(&self, tx: mpsc::UnboundedSender<ClipboardEvent>) {
        let last_text = self.last_text.clone();
        let enabled = self.enabled.clone();

        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(500));

            if !*unlock(&enabled) {
                continue;
            }

            let mut clipboard = match Clipboard::new() {
                Ok(cb) => cb,
                Err(e) => {
                    warn!("Clipboard access failed: {}", e);
                    continue;
                }
            };

            if let Ok(text) = clipboard.get_text() {
                let mut last = unlock(&last_text);
                if !text.is_empty() && text != *last {
                    *last = text.clone();
                    let _ = tx.send(ClipboardEvent::NewText(text));
                }
            }
        });
    }

    #[allow(dead_code)]
    pub fn read_text() -> Option<String> {
        Clipboard::new().ok()?.get_text().ok()
    }

    pub fn write_text(text: &str) -> bool {
        match Clipboard::new() {
            Ok(mut cb) => cb.set_text(text).is_ok(),
            Err(_) => false,
        }
    }
}
