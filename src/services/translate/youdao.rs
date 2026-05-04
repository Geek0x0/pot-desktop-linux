use crate::services::translate::TranslateService;
use crate::services::types::{ServiceError, TranslateRequest, TranslateResult};
use sha2::{Digest, Sha256};

pub struct YoudaoTranslate;

fn map_lang(lang: &str) -> &str {
    match lang {
        "zh_cn" => "zh-CHS",
        "zh_tw" => "zh-CHT",
        "en" => "en",
        "ja" => "ja",
        "ko" => "ko",
        "fr" => "fr",
        "es" => "es",
        "de" => "de",
        "ru" => "ru",
        "it" => "it",
        "pt" => "pt",
        "ar" => "ar",
        "th" => "th",
        "vi" => "vi",
        "id" => "id",
        "ms" => "ms",
        "hi" => "hi",
        "auto" => "auto",
        _ => lang,
    }
}

fn truncate(text: &str) -> String {
    let len = text.chars().count();
    if len <= 20 {
        text.to_string()
    } else {
        let head: String = text.chars().take(10).collect();
        let tail: String = text.chars().skip(len - 10).collect();
        format!("{}{}{}", head, len, tail)
    }
}

#[async_trait::async_trait]
impl TranslateService for YoudaoTranslate {
    fn name(&self) -> &str {
        "youdao"
    }

    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResult, ServiceError> {
        let appkey = req
            .config
            .get("appkey")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let key = req.config.get("key").and_then(|v| v.as_str()).unwrap_or("");

        if appkey.is_empty() || key.is_empty() {
            return Err(ServiceError {
                service: "youdao".into(),
                message: "appkey or key not configured".into(),
            });
        }

        let salt = crate::util::nanoid(8);
        let curtime = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| ServiceError {
                service: "youdao".into(),
                message: e.to_string(),
            })?
            .as_secs()
            .to_string();

        let sign_input = format!(
            "{}{}{}{}{}",
            appkey,
            truncate(&req.text),
            salt,
            curtime,
            key
        );
        let mut hasher = Sha256::new();
        hasher.update(sign_input.as_bytes());
        let sign = format!("{:x}", hasher.finalize());

        let from = map_lang(&req.from);
        let to = map_lang(&req.to);

        let url = format!(
            "https://openapi.youdao.com/api?q={}&from={}&to={}&appKey={}&salt={}&sign={}&signType=v3&curtime={}",
            urlencoding::encode(&req.text), from, to, appkey, salt, sign, curtime
        );

        let client = super::http_client();
        let resp: serde_json::Value = client
            .get(&url)
            .send()
            .await
            .map_err(|e| ServiceError {
                service: "youdao".into(),
                message: e.to_string(),
            })?
            .json()
            .await
            .map_err(|e| ServiceError {
                service: "youdao".into(),
                message: e.to_string(),
            })?;

        let error_code = resp
            .get("errorCode")
            .and_then(|v| v.as_str())
            .unwrap_or("0");
        if error_code != "0" {
            return Err(ServiceError {
                service: "youdao".into(),
                message: format!("Error code: {}", error_code),
            });
        }

        // Check for dictionary mode
        if resp
            .get("isWord")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Ok(TranslateResult::Dictionary(self.parse_dictionary(&resp)));
        }

        // Translation mode
        let mut result = String::new();
        if let Some(translations) = resp.get("translation").and_then(|v| v.as_array()) {
            for item in translations {
                if let Some(text) = item.as_str() {
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    result.push_str(text);
                }
            }
        }

        if result.is_empty() {
            return Err(ServiceError {
                service: "youdao".into(),
                message: "No translation result".into(),
            });
        }

        Ok(TranslateResult::Text(result))
    }

    fn default_config(&self) -> serde_json::Value {
        serde_json::json!({ "appkey": "", "key": "" })
    }
}

impl YoudaoTranslate {
    fn parse_dictionary(&self, resp: &serde_json::Value) -> super::super::types::DictionaryResult {
        use super::super::types::*;

        let mut pronunciations = Vec::new();
        if let Some(basic) = resp.get("basic") {
            if let Some(uk) = basic.get("uk-phonetic").and_then(|v| v.as_str()) {
                pronunciations.push(Pronunciation {
                    region: "UK".into(),
                    symbol: uk.to_string(),
                });
            }
            if let Some(us) = basic.get("us-phonetic").and_then(|v| v.as_str()) {
                pronunciations.push(Pronunciation {
                    region: "US".into(),
                    symbol: us.to_string(),
                });
            }
        }

        let mut explanations = Vec::new();
        if let Some(basic) = resp.get("basic") {
            if let Some(explains) = basic.get("explains").and_then(|v| v.as_array()) {
                let mut explains_list = Vec::new();
                for exp in explains {
                    if let Some(text) = exp.as_str() {
                        explains_list.push(Explain {
                            text: text.to_string(),
                        });
                    }
                }
                if !explains_list.is_empty() {
                    explanations.push(Explanation {
                        trait_name: String::new(),
                        explains: explains_list,
                    });
                }
            }
        }

        let mut associations = Vec::new();
        if let Some(web) = resp.get("web").and_then(|v| v.as_array()) {
            for item in web.iter().take(5) {
                if let Some(key) = item.get("key").and_then(|v| v.as_str()) {
                    associations.push(key.to_string());
                }
            }
        }

        let sentences = Vec::new();

        DictionaryResult {
            pronunciations,
            explanations,
            associations,
            sentences,
        }
    }
}
