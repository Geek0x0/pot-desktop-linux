use crate::services::translate::TranslateService;
use crate::services::types::{ServiceError, TranslateRequest, TranslateResult};
use md5::{Digest, Md5};

pub struct BaiduTranslate;

fn map_lang(lang: &str) -> &str {
    match lang {
        "zh_cn" => "zh",
        "zh_tw" => "cht",
        "en" => "en",
        "ja" => "jp",
        "ko" => "kor",
        "fr" => "fra",
        "es" => "spa",
        "de" => "de",
        "ru" => "ru",
        "it" => "it",
        "pt" => "pt",
        "ar" => "ara",
        "th" => "th",
        "vi" => "vie",
        "id" => "id",
        "ms" => "may",
        "hi" => "hi",
        "auto" => "auto",
        _ => lang,
    }
}

#[async_trait::async_trait]
impl TranslateService for BaiduTranslate {
    fn name(&self) -> &str {
        "baidu"
    }

    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResult, ServiceError> {
        let appid = req
            .config
            .get("appid")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let secret = req
            .config
            .get("secret")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if appid.is_empty() || secret.is_empty() {
            return Err(ServiceError {
                service: "baidu".into(),
                message: "appid or secret not configured".into(),
            });
        }

        let salt = crate::util::nanoid(8);
        let sign_input = format!("{}{}{}{}", appid, req.text, salt, secret);
        let mut hasher = Md5::new();
        hasher.update(sign_input.as_bytes());
        let sign = format!("{:x}", hasher.finalize());

        let from = map_lang(&req.from);
        let to = map_lang(&req.to);

        let url = format!(
            "https://fanyi-api.baidu.com/api/trans/vip/translate?q={}&from={}&to={}&appid={}&salt={}&sign={}",
            urlencoding::encode(&req.text), from, to, appid, salt, sign
        );

        let client = super::http_client();
        let resp: serde_json::Value = client
            .get(&url)
            .send()
            .await
            .map_err(|e| ServiceError {
                service: "baidu".into(),
                message: e.to_string(),
            })?
            .json()
            .await
            .map_err(|e| ServiceError {
                service: "baidu".into(),
                message: e.to_string(),
            })?;

        if let Some(err_code) = resp.get("error_code").and_then(|v| v.as_str()) {
            let err_msg = resp
                .get("error_msg")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            return Err(ServiceError {
                service: "baidu".into(),
                message: format!("{}: {}", err_code, err_msg),
            });
        }

        let mut result = String::new();
        if let Some(trans) = resp.get("trans_result").and_then(|v| v.as_array()) {
            for item in trans {
                if let Some(dst) = item.get("dst").and_then(|v| v.as_str()) {
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    result.push_str(dst);
                }
            }
        }

        if result.is_empty() {
            return Err(ServiceError {
                service: "baidu".into(),
                message: "No translation result".into(),
            });
        }

        Ok(TranslateResult::Text(result))
    }

    fn default_config(&self) -> serde_json::Value {
        serde_json::json!({ "appid": "", "secret": "" })
    }
}
