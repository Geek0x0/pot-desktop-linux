pub mod baidu;
pub mod bing;
pub mod deepl;
pub mod google;
pub mod lingva;
pub mod openai;
pub mod youdao;

use crate::services::types::{ServiceError, TranslateRequest, TranslateResult};
use std::sync::LazyLock;

use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|e| {
            log::error!("Failed to create HTTP client: {}", e);
            // Fall back to a minimal client — better than panicking.
            Client::new()
        })
});

pub fn http_client() -> &'static Client {
    &HTTP_CLIENT
}

#[async_trait::async_trait]
pub trait TranslateService: Send + Sync {
    fn name(&self) -> &str;
    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResult, ServiceError>;
    fn default_config(&self) -> Value;
}

pub struct TranslateRegistry {
    services: HashMap<String, Arc<dyn TranslateService>>,
}

impl TranslateRegistry {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    pub fn register(&mut self, service: Arc<dyn TranslateService>) {
        self.services.insert(service.name().to_string(), service);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn TranslateService>> {
        self.services.get(name)
    }

    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.services.values().map(|s| s.name()).collect();
        names.sort();
        names
    }

    pub async fn translate(
        &self,
        service_name: &str,
        req: TranslateRequest,
    ) -> Result<TranslateResult, ServiceError> {
        self.get(service_name)
            .ok_or_else(|| ServiceError {
                service: service_name.to_string(),
                message: "Service not found".into(),
            })?
            .translate(req)
            .await
    }
}

pub fn create_registry() -> TranslateRegistry {
    let mut reg = TranslateRegistry::new();

    reg.register(Arc::new(google::GoogleTranslate));
    reg.register(Arc::new(bing::BingTranslate));
    reg.register(Arc::new(deepl::DeepLTranslate));
    reg.register(Arc::new(openai::OpenAITranslate));
    reg.register(Arc::new(baidu::BaiduTranslate));
    reg.register(Arc::new(youdao::YoudaoTranslate));
    reg.register(Arc::new(lingva::LingvaTranslate));

    reg
}

pub fn create_registry_with_plugins(
    plugin_manager: &crate::services::plugin::PluginManager,
) -> TranslateRegistry {
    let mut reg = create_registry();

    for adapter in crate::services::plugin::create_translate_adapters(plugin_manager) {
        reg.register(adapter);
    }

    reg
}
