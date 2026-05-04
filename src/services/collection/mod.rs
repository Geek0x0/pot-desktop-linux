use crate::error::AppError;
use std::collections::HashMap;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone)]
pub struct CollectionRequest {
    pub source_text: String,
    pub result_text: String,
    pub source_lang: String,
    pub result_lang: String,
    pub config: serde_json::Value,
}

#[derive(Debug)]
pub struct CollectionResult {
    pub success: bool,
    pub message: String,
}

#[async_trait::async_trait]
pub trait CollectionService: Send + Sync {
    fn name(&self) -> &str;
    async fn collect(&self, request: CollectionRequest) -> Result<CollectionResult>;
    fn default_config(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

pub struct CollectionRegistry {
    services: HashMap<String, Box<dyn CollectionService>>,
}

impl CollectionRegistry {
    pub fn new() -> Self {
        CollectionRegistry {
            services: HashMap::new(),
        }
    }

    pub fn register(&mut self, service: Box<dyn CollectionService>) {
        self.services.insert(service.name().to_string(), service);
    }

    pub fn get(&self, name: &str) -> Option<&dyn CollectionService> {
        self.services.get(name).map(|s| s.as_ref())
    }

    pub async fn collect(
        &self,
        name: &str,
        request: CollectionRequest,
    ) -> Result<CollectionResult> {
        let service = self
            .services
            .get(name)
            .ok_or_else(|| AppError::Custom(format!("Unknown collection service: {}", name)))?;
        service.collect(request).await
    }

    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.services.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }
}

pub fn create_registry() -> CollectionRegistry {
    let mut registry = CollectionRegistry::new();
    registry.register(Box::new(AnkiCollection));
    registry
}

/// Anki collection backend — sends cards via AnkiConnect
struct AnkiCollection;

#[async_trait::async_trait]
impl CollectionService for AnkiCollection {
    fn name(&self) -> &str {
        "anki"
    }

    async fn collect(&self, request: CollectionRequest) -> Result<CollectionResult> {
        let host = request
            .config
            .get("requestPath")
            .and_then(|v| v.as_str())
            .unwrap_or("http://127.0.0.1:8765");

        let deck = request
            .config
            .get("deck")
            .and_then(|v| v.as_str())
            .unwrap_or("Pot");

        let model = request
            .config
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("Basic");

        let client = crate::services::translate::http_client();

        let body = serde_json::json!({
            "action": "addNote",
            "version": 6,
            "params": {
                "note": {
                    "deckName": deck,
                    "modelName": model,
                    "fields": {
                        "Front": request.source_text,
                        "Back": request.result_text
                    },
                    "tags": ["pot"]
                }
            }
        });

        let resp = client
            .post(host)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Custom(format!("AnkiConnect request failed: {}", e)))?;

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Custom(format!("AnkiConnect response error: {}", e)))?;

        let error = json.get("error").and_then(|e| e.as_str());
        match error {
            Some(err) if !err.is_empty() => Ok(CollectionResult {
                success: false,
                message: format!("Anki error: {}", err),
            }),
            _ => Ok(CollectionResult {
                success: true,
                message: "Added to Anki".into(),
            }),
        }
    }

    fn default_config(&self) -> serde_json::Value {
        serde_json::json!({
            "requestPath": "http://127.0.0.1:8765",
            "deck": "Pot",
            "model": "Basic"
        })
    }
}
