use crate::services::translate::TranslateService;
use crate::services::types::{ServiceError, TranslateRequest, TranslateResult};

pub struct OpenAITranslate;

#[async_trait::async_trait]
impl TranslateService for OpenAITranslate {
    fn name(&self) -> &str {
        "openai"
    }

    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResult, ServiceError> {
        let api_key = req
            .config
            .get("api_key")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let model = req
            .config
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let request_url = req
            .config
            .get("request_url")
            .and_then(|v| v.as_str())
            .or_else(|| req.config.get("request_path").and_then(|v| v.as_str()))
            .unwrap_or("");
        let api_format = req
            .config
            .get("api_format")
            .and_then(|v| v.as_str())
            .unwrap_or("openai_compatible");
        let legacy_service = req
            .config
            .get("service")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let extra_args = req
            .config
            .get("request_arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        if api_key.trim().is_empty() {
            return Err(ServiceError {
                service: "openai".into(),
                message: "API key is required".into(),
            });
        }
        if model.trim().is_empty() {
            return Err(ServiceError {
                service: "openai".into(),
                message: "Model is required".into(),
            });
        }

        let prompt_text = build_prompt(&req);

        match api_format {
            "anthropic" => {
                translate_anthropic(request_url, api_key, model, prompt_text, extra_args).await
            }
            _ => {
                translate_openai_compatible(
                    request_url,
                    api_key,
                    model,
                    prompt_text,
                    extra_args,
                    legacy_service == "azure",
                )
                .await
            }
        }
    }

    fn default_config(&self) -> serde_json::Value {
        serde_json::json!({
            "api_format": "openai_compatible",
            "api_key": "",
            "model": "gpt-3.5-turbo",
            "request_url": "https://api.openai.com/v1/chat/completions",
            "prompt": [],
            "request_arguments": {}
        })
    }
}

fn build_prompt(req: &TranslateRequest) -> String {
    let prompt_list = req
        .config
        .get("prompt")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut content_parts = Vec::new();
    for msg in &prompt_list {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let content = msg
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .replace("$text", &req.text)
            .replace("$from", &req.from)
            .replace("$to", &req.to);
        if !content.trim().is_empty() {
            content_parts.push(format!("{}: {}", role, content));
        }
    }

    if content_parts.is_empty() {
        format!(
            "Translate the following text from {} to {}:\n\n{}",
            req.from, req.to, req.text
        )
    } else {
        content_parts.join("\n\n")
    }
}

async fn translate_openai_compatible(
    request_url: &str,
    api_key: &str,
    model: &str,
    prompt_text: String,
    extra_args: serde_json::Value,
    use_api_key_header: bool,
) -> Result<TranslateResult, ServiceError> {
    let url = normalize_endpoint(
        request_url,
        "https://api.openai.com/v1/chat/completions",
        "chat/completions",
    );
    let messages = vec![serde_json::json!({ "role": "user", "content": prompt_text })];

    let mut body = serde_json::json!({
        "messages": messages,
        "model": model,
        "stream": false,
    });

    merge_extra_args(&mut body, &extra_args);

    let client = super::http_client();
    let mut request = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body);

    request = if use_api_key_header {
        request.header("api-key", api_key)
    } else {
        request.header("Authorization", format!("Bearer {}", api_key))
    };

    let resp = send_json(request, "openai").await?;

    if let Some(err) = resp.get("error") {
        return Err(ServiceError {
            service: "openai".into(),
            message: extract_error_message(err),
        });
    }

    let text = resp
        .get("choices")
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("message"))
        .and_then(|v| v.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if text.is_empty() {
        return Err(ServiceError {
            service: "openai".into(),
            message: "No translation result".into(),
        });
    }

    Ok(TranslateResult::Text(text))
}

async fn translate_anthropic(
    request_url: &str,
    api_key: &str,
    model: &str,
    prompt_text: String,
    extra_args: serde_json::Value,
) -> Result<TranslateResult, ServiceError> {
    let url = normalize_endpoint(
        request_url,
        "https://api.anthropic.com/v1/messages",
        "messages",
    );

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "messages": [
            { "role": "user", "content": prompt_text }
        ],
        "stream": false,
    });

    merge_extra_args(&mut body, &extra_args);

    let client = super::http_client();
    let request = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body);

    let resp = send_json(request, "openai").await?;

    if let Some(err) = resp.get("error") {
        return Err(ServiceError {
            service: "openai".into(),
            message: extract_error_message(err),
        });
    }

    let text = resp
        .get("content")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(|v| v.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
        .trim()
        .to_string();

    if text.is_empty() {
        return Err(ServiceError {
            service: "openai".into(),
            message: "No translation result".into(),
        });
    }

    Ok(TranslateResult::Text(text))
}

fn normalize_endpoint(input: &str, default_url: &str, suffix: &str) -> String {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return default_url.to_string();
    }

    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return if trimmed.starts_with('/') {
            let host = default_url
                .split("/v1")
                .next()
                .unwrap_or("https://api.openai.com");
            format!("{}{}", host, trimmed)
        } else {
            trimmed.to_string()
        };
    }

    let suffix = suffix.trim_start_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{}/{}", trimmed, suffix)
    } else if trimmed.ends_with("/models") {
        format!(
            "{}/{}",
            trimmed.trim_end_matches("/models").trim_end_matches('/'),
            suffix
        )
    } else {
        trimmed.to_string()
    }
}

fn merge_extra_args(body: &mut serde_json::Value, extra_args: &serde_json::Value) {
    if let Some(obj) = extra_args.as_object() {
        if let Some(body_obj) = body.as_object_mut() {
            for (k, v) in obj {
                body_obj.insert(k.clone(), v.clone());
            }
        }
    }
}

async fn send_json(
    request: reqwest::RequestBuilder,
    service: &str,
) -> Result<serde_json::Value, ServiceError> {
    let response = request.send().await.map_err(|e| ServiceError {
        service: service.into(),
        message: e.to_string(),
    })?;
    let status = response.status();
    let body = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| ServiceError {
            service: service.into(),
            message: e.to_string(),
        })?;

    if !status.is_success() {
        let message = body
            .get("error")
            .map(extract_error_message)
            .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
        return Err(ServiceError {
            service: service.into(),
            message,
        });
    }

    Ok(body)
}

fn extract_error_message(err: &serde_json::Value) -> String {
    err.get("message")
        .and_then(|v| v.as_str())
        .or_else(|| err.as_str())
        .unwrap_or("Unknown API error")
        .to_string()
}
