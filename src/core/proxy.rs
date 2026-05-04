use crate::config::AppConfig;

pub fn set_proxy(config: &AppConfig) -> Result<bool, String> {
    let host = match config.get("proxy_host") {
        Some(v) => v.as_str().unwrap_or_default().to_string(),
        None => return Err("proxy_host is not set".into()),
    };
    if host.is_empty() {
        return Err("proxy_host is not set".into());
    }
    let port = config
        .get("proxy_port")
        .map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default();
    let no_proxy = match config.get("no_proxy") {
        Some(v) => v.as_str().unwrap_or("").to_string(),
        None => String::new(),
    };
    let proxy = format!("http://{}:{}", host, port);

    std::env::set_var("http_proxy", &proxy);
    std::env::set_var("https_proxy", &proxy);
    std::env::set_var("all_proxy", &proxy);
    std::env::set_var("no_proxy", &no_proxy);
    log::info!("Proxy set to {}", proxy);
    Ok(true)
}

#[allow(dead_code)]
pub fn unset_proxy() -> Result<bool, String> {
    std::env::remove_var("http_proxy");
    std::env::remove_var("https_proxy");
    std::env::remove_var("all_proxy");
    std::env::remove_var("no_proxy");
    Ok(true)
}
