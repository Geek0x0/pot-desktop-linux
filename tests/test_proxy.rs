use pot_gtk::core::proxy;

struct TestConfigContext {
    _dir: tempfile::TempDir,
    config: pot_gtk::config::AppConfig,
}

fn test_config() -> TestConfigContext {
    let dir = tempfile::tempdir().unwrap();
    let config = pot_gtk::config::AppConfig::new_in_dir(dir.path().join("com.pot-app.desktop"));
    TestConfigContext { _dir: dir, config }
}

#[test]
fn unset_proxy_removes_env_vars() {
    std::env::set_var("http_proxy", "http://test:8080");
    std::env::set_var("https_proxy", "http://test:8080");
    let _ = proxy::unset_proxy();
    assert!(
        std::env::var("http_proxy").is_err() || std::env::var("http_proxy").unwrap().is_empty()
    );
    assert!(
        std::env::var("https_proxy").is_err() || std::env::var("https_proxy").unwrap().is_empty()
    );
}

#[test]
fn set_proxy_sets_env_vars() {
    let _ = proxy::unset_proxy();
    let ctx = test_config();
    ctx.config.set("proxy_host", "127.0.0.1");
    ctx.config.set("proxy_port", 9090);
    ctx.config.set("no_proxy", "localhost,127.0.0.1");

    let result = proxy::set_proxy(&ctx.config);
    assert!(result.is_ok());

    let http = std::env::var("http_proxy").unwrap_or_default();
    assert!(
        http.contains("127.0.0.1:9090"),
        "http_proxy should contain host:port, got: {}",
        http
    );

    let no_proxy = std::env::var("no_proxy").unwrap_or_default();
    assert!(
        no_proxy.contains("localhost"),
        "no_proxy should contain localhost, got: {}",
        no_proxy
    );

    let _ = proxy::unset_proxy();
}

#[test]
fn set_proxy_empty_host_returns_err() {
    let _ = proxy::unset_proxy();
    let ctx = test_config();
    ctx.config.set("proxy_host", "");
    let result = proxy::set_proxy(&ctx.config);
    assert!(
        result.is_err(),
        "set_proxy with empty host should return Err"
    );
}

#[test]
fn unset_proxy_cleans_up() {
    std::env::set_var("all_proxy", "http://test:3128");
    let _ = proxy::unset_proxy();
    assert!(std::env::var("all_proxy").is_err());
}
