use crate::services::translate::TranslateService;
use crate::services::types::{ServiceError, TranslateRequest, TranslateResult};

pub struct LingvaTranslate;

#[async_trait::async_trait]
impl TranslateService for LingvaTranslate {
    fn name(&self) -> &str {
        "lingva"
    }

    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResult, ServiceError> {
        let base_url = req
            .config
            .get("custom_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://lingva.pot-app.com");

        let encoded_text = req.text.replace('/', "@@");
        let url = format!(
            "{}/api/v1/{}/{}/{}",
            base_url,
            req.from,
            req.to,
            urlencoding::encode(&encoded_text)
        );

        let client = super::http_client();
        let resp: serde_json::Value = client
            .get(&url)
            .send()
            .await
            .map_err(|e| ServiceError {
                service: self.name().into(),
                message: e.to_string(),
            })?
            .json()
            .await
            .map_err(|e| ServiceError {
                service: self.name().into(),
                message: e.to_string(),
            })?;

        let text = resp
            .get("translation")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if text.is_empty() {
            return Err(ServiceError {
                service: self.name().into(),
                message: "No translation result".into(),
            });
        }

        Ok(TranslateResult::Text(text))
    }

    fn default_config(&self) -> serde_json::Value {
        serde_json::json!({ "custom_url": "https://lingva.pot-app.com" })
    }
}
