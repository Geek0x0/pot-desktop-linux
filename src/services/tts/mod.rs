use crate::error::AppError;
use std::collections::HashMap;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone)]
pub struct TtsRequest {
    pub text: String,
    pub language: String,
    pub config: serde_json::Value,
}

#[derive(Debug)]
pub struct TtsResult {
    pub audio_base64: String,
}

#[async_trait::async_trait]
pub trait TtsService: Send + Sync {
    fn name(&self) -> &str;
    async fn speak(&self, request: TtsRequest) -> Result<TtsResult>;
    fn default_config(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

pub struct TtsRegistry {
    services: HashMap<String, Box<dyn TtsService>>,
}

impl TtsRegistry {
    pub fn new() -> Self {
        TtsRegistry {
            services: HashMap::new(),
        }
    }

    pub fn register(&mut self, service: Box<dyn TtsService>) {
        self.services.insert(service.name().to_string(), service);
    }

    pub fn get(&self, name: &str) -> Option<&dyn TtsService> {
        self.services.get(name).map(|s| s.as_ref())
    }

    pub async fn speak(&self, name: &str, request: TtsRequest) -> Result<TtsResult> {
        let service = self
            .services
            .get(name)
            .ok_or_else(|| AppError::Custom(format!("Unknown TTS service: {}", name)))?;
        service.speak(request).await
    }

    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.services.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }
}

pub fn create_registry() -> TtsRegistry {
    let mut registry = TtsRegistry::new();
    registry.register(Box::new(LingvaTts));
    registry
}

/// Lingva TTS backend — uses the Lingva API to generate audio
struct LingvaTts;

#[async_trait::async_trait]
impl TtsService for LingvaTts {
    fn name(&self) -> &str {
        "lingva_tts"
    }

    async fn speak(&self, request: TtsRequest) -> Result<TtsResult> {
        let host = request
            .config
            .get("requestPath")
            .and_then(|v| v.as_str())
            .unwrap_or("lingva.pot-app.com");

        let scheme = if host.starts_with("http://") || host.starts_with("https://") {
            ""
        } else {
            "https://"
        };

        let lang = map_language(&request.language);
        let url = format!(
            "{}{}/api/v1/audio/{}/{}",
            scheme,
            host,
            lang,
            urlencoding::encode(&request.text)
        );

        let client = crate::services::translate::http_client();

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Custom(format!("TTS request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::Custom(format!(
                "TTS API returned status {}",
                resp.status()
            )));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Custom(format!("TTS response parse error: {}", e)))?;

        let audio = json["audio"]
            .as_str()
            .ok_or_else(|| AppError::Custom("TTS response missing 'audio' field".into()))?;

        Ok(TtsResult {
            audio_base64: audio.to_string(),
        })
    }

    fn default_config(&self) -> serde_json::Value {
        serde_json::json!({
            "requestPath": "lingva.pot-app.com"
        })
    }
}

fn map_language(lang: &str) -> &str {
    match lang {
        "zh_cn" | "zh_tw" => "zh",
        "en" => "en",
        "ja" => "ja",
        "ko" => "ko",
        "fr" => "fr",
        "de" => "de",
        "es" => "es",
        "ru" => "ru",
        "it" => "it",
        "pt_pt" | "pt_br" => "pt",
        "tr" => "tr",
        "ar" => "ar",
        _ => "en",
    }
}
