use crate::services::translate::TranslateService;
use crate::services::types::{ServiceError, TranslateRequest, TranslateResult};
use std::sync::{LazyLock, Mutex};

struct CachedToken {
    auth_url: String,
    token: String,
    expires_at: std::time::Instant,
}

static TOKEN_CACHE: LazyLock<Mutex<Option<CachedToken>>> = LazyLock::new(|| Mutex::new(None));

pub struct BingTranslate;

impl BingTranslate {
    async fn get_token(auth_url: &str) -> Result<String, ServiceError> {
        {
            let cache = TOKEN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(cached) = cache.as_ref() {
                if cached.auth_url == auth_url && cached.expires_at > std::time::Instant::now() {
                    return Ok(cached.token.clone());
                }
            }
        }

        let client = super::http_client();
        let token: String = client
            .get(auth_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0")
            .send()
            .await
            .map_err(|e| ServiceError { service: "bing".into(), message: format!("Auth request failed: {}", e) })?
            .text()
            .await
            .map_err(|e| ServiceError { service: "bing".into(), message: format!("Auth response failed: {}", e) })?;

        let mut cache = TOKEN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        *cache = Some(CachedToken {
            auth_url: auth_url.to_string(),
            token: token.clone(),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(540), // 9 min
        });

        Ok(token)
    }
}

fn map_lang(lang: &str) -> &str {
    match lang {
        "zh_cn" => "zh-Hans",
        "zh_tw" => "zh-Hant",
        other => other,
    }
}

#[async_trait::async_trait]
impl TranslateService for BingTranslate {
    fn name(&self) -> &str {
        "bing"
    }

    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResult, ServiceError> {
        let auth_url = req
            .config
            .get("auth_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://edge.microsoft.com/translate/auth");
        let request_url = req
            .config
            .get("request_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://api-edge.cognitive.microsofttranslator.com/translate");

        let token = Self::get_token(auth_url).await?;
        let from = map_lang(&req.from);
        let to = map_lang(&req.to);

        let url = if req.from == "auto" || req.from.is_empty() {
            format!("{}?to={}&api-version=3.0", request_url, to)
        } else {
            format!("{}?from={}&to={}&api-version=3.0", request_url, from, to)
        };

        let body = serde_json::json!([{ "Text": req.text }]);

        let client = super::http_client();
        let resp: serde_json::Value = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
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

        if let Some(err) = resp.get("error") {
            let message = err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown API error");
            return Err(ServiceError {
                service: self.name().into(),
                message: message.to_string(),
            });
        }

        let text = resp
            .get(0)
            .and_then(|v| v.get("translations"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("text"))
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
        serde_json::json!({})
    }
}
