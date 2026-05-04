use crate::services::translate::TranslateService;
use crate::services::types::{ServiceError, TranslateRequest, TranslateResult};

pub struct DeepLTranslate;

#[async_trait::async_trait]
impl TranslateService for DeepLTranslate {
    fn name(&self) -> &str {
        "deepl"
    }

    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResult, ServiceError> {
        let mode = req
            .config
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("free");

        match mode {
            "api" => self.translate_api(&req).await,
            "deeplx" => self.translate_deeplx(&req).await,
            _ => self.translate_free(&req).await,
        }
    }

    fn default_config(&self) -> serde_json::Value {
        serde_json::json!({ "type": "free", "auth_key": "", "custom_url": "" })
    }
}

impl DeepLTranslate {
    async fn translate_free(
        &self,
        req: &TranslateRequest,
    ) -> Result<TranslateResult, ServiceError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| ServiceError {
                service: "deepl".into(),
                message: e.to_string(),
            })?;

        let id = now.as_millis() as u64;

        let i_count = req.text.matches('i').count();
        let ts = now.as_millis() as u64;

        let timestamp = if i_count > 0 { ts + i_count as u64 } else { ts };

        let method_str = if i_count > 0 {
            r#""method" : ""#
        } else {
            r#""method":""#
        };

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "LMT_handle_texts",
            "params": {
                "splitting": "newlines",
                "texts": [{ "text": req.text }],
                "lang": {
                    "source_lang_user_selected": req.from,
                    "target_lang": req.to
                },
                "timestamp": timestamp
            },
            "id": id
        });

        let body_str = serde_json::to_string(&body).map_err(|e| ServiceError {
            service: "deepl".into(),
            message: e.to_string(),
        })?;
        let body_str = body_str.replace(r#""method":""#, method_str);

        let client = super::http_client();
        let resp: serde_json::Value = client
            .post("https://www2.deepl.com/jsonrpc")
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .await
            .map_err(|e| ServiceError {
                service: "deepl".into(),
                message: e.to_string(),
            })?
            .json()
            .await
            .map_err(|e| ServiceError {
                service: "deepl".into(),
                message: e.to_string(),
            })?;

        let text = resp
            .get("result")
            .and_then(|v| v.get("texts"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if text.is_empty() {
            return Err(ServiceError {
                service: "deepl".into(),
                message: "No translation result".into(),
            });
        }
        Ok(TranslateResult::Text(text))
    }

    async fn translate_api(&self, req: &TranslateRequest) -> Result<TranslateResult, ServiceError> {
        let auth_key = req
            .config
            .get("auth_key")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let base = if auth_key.ends_with(":fx") {
            "https://api-free.deepl.com/v2/translate"
        } else {
            "https://api.deepl.com/v2/translate"
        };

        let body = serde_json::json!({
            "text": [req.text],
            "target_lang": req.to,
            "source_lang": req.from
        });

        let client = super::http_client();
        let resp: serde_json::Value = client
            .post(base)
            .header("Authorization", format!("DeepL-Auth-Key {}", auth_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ServiceError {
                service: "deepl".into(),
                message: e.to_string(),
            })?
            .json()
            .await
            .map_err(|e| ServiceError {
                service: "deepl".into(),
                message: e.to_string(),
            })?;

        let text = resp
            .get("translations")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if text.is_empty() {
            return Err(ServiceError {
                service: "deepl".into(),
                message: "No translation result".into(),
            });
        }
        Ok(TranslateResult::Text(text))
    }

    async fn translate_deeplx(
        &self,
        req: &TranslateRequest,
    ) -> Result<TranslateResult, ServiceError> {
        let custom_url = req
            .config
            .get("custom_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if custom_url.is_empty() {
            return Err(ServiceError {
                service: "deepl".into(),
                message: "DeepLX URL not configured".into(),
            });
        }

        let body = serde_json::json!({
            "source_lang": req.from,
            "target_lang": req.to,
            "text": req.text
        });

        let client = super::http_client();
        let resp: serde_json::Value = client
            .post(custom_url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ServiceError {
                service: "deepl".into(),
                message: e.to_string(),
            })?
            .json()
            .await
            .map_err(|e| ServiceError {
                service: "deepl".into(),
                message: e.to_string(),
            })?;

        let text = resp
            .get("data")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if text.is_empty() {
            return Err(ServiceError {
                service: "deepl".into(),
                message: "No translation result".into(),
            });
        }
        Ok(TranslateResult::Text(text))
    }
}
