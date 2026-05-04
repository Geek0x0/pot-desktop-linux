use crate::services::translate::TranslateService;
use crate::services::types::{ServiceError, TranslateRequest, TranslateResult};

pub struct GoogleTranslate;

#[async_trait::async_trait]
impl TranslateService for GoogleTranslate {
    fn name(&self) -> &str {
        "google"
    }

    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResult, ServiceError> {
        let custom_url = req
            .config
            .get("custom_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://translate.google.com");

        let url = format!(
            "{}/translate_a/single?client=gtx&sl={}&tl={}&hl={}&dt=t&ie=UTF-8&oe=UTF-8&q={}",
            custom_url,
            req.from,
            req.to,
            req.to,
            urlencoding::encode(&req.text)
        );

        let client = super::http_client();
        let resp: serde_json::Value = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
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

        // Extract translated text from nested array: result[0][*][0]
        let mut translated = String::new();
        if let Some(arr) = resp.get(0).and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(text) = item.get(0).and_then(|v| v.as_str()) {
                    translated.push_str(text);
                }
            }
        }

        // Check for dictionary mode (result[1] exists and is an array)
        if let Some(dict_data) = resp.get(1) {
            if dict_data.is_array() {
                return Ok(TranslateResult::Dictionary(parse_dictionary(
                    dict_data, &resp,
                )));
            }
        }

        if translated.is_empty() {
            return Err(ServiceError {
                service: self.name().into(),
                message: "No translation result".into(),
            });
        }

        Ok(TranslateResult::Text(translated))
    }

    fn default_config(&self) -> serde_json::Value {
        serde_json::json!({ "custom_url": "https://translate.google.com" })
    }
}

fn parse_dictionary(
    dict: &serde_json::Value,
    full: &serde_json::Value,
) -> super::super::types::DictionaryResult {
    use super::super::types::*;

    let mut pronunciations = Vec::new();
    if let Some(phonetic) = full
        .get(0)
        .and_then(|a| a.get(0))
        .and_then(|a| a.get(1))
        .and_then(|v| v.as_array())
    {
        if phonetic.len() >= 4 {
            pronunciations.push(Pronunciation {
                region: "source".into(),
                symbol: phonetic[3].as_str().unwrap_or("").into(),
            });
        }
    }

    let mut explanations = Vec::new();
    if let Some(groups) = dict.as_array() {
        for group in groups {
            if let Some(parts) = group.as_array() {
                if parts.len() >= 2 {
                    let trait_name = parts[0].as_str().unwrap_or("").to_string();
                    let mut explains_list = Vec::new();
                    if let Some(terms) = parts[2].as_array() {
                        for term in terms {
                            if let Some(arr) = term.as_array() {
                                if let Some(text) = arr.first().and_then(|v| v.as_str()) {
                                    explains_list.push(Explain {
                                        text: text.to_string(),
                                    });
                                }
                            }
                        }
                    }
                    if !explains_list.is_empty() {
                        explanations.push(Explanation {
                            trait_name,
                            explains: explains_list,
                        });
                    }
                }
            }
        }
    }

    let associations = Vec::new();
    let sentences = Vec::new();

    DictionaryResult {
        pronunciations,
        explanations,
        associations,
        sentences,
    }
}
