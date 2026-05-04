use pot_gtk::config::{AppConfig, ServiceCategory};

struct TestConfigContext {
    _dir: tempfile::TempDir,
    config: AppConfig,
}

fn test_config() -> TestConfigContext {
    let dir = tempfile::tempdir().unwrap();
    let config = AppConfig::new_in_dir(dir.path().join("com.pot-app.desktop"));
    TestConfigContext { _dir: dir, config }
}

#[test]
fn get_returns_none_for_missing_key() {
    let ctx = test_config();
    assert!(ctx.config.get("nonexistent_key_xyz").is_none());
}

#[test]
fn set_and_get_string() {
    let ctx = test_config();
    ctx.config.set("test_key", "hello");
    assert_eq!(
        ctx.config
            .get("test_key")
            .and_then(|v| v.as_str().map(String::from)),
        Some("hello".into())
    );
}

#[test]
fn set_and_get_bool() {
    let ctx = test_config();
    ctx.config.set("test_bool", true);
    assert_eq!(
        ctx.config.get("test_bool").and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[test]
fn set_and_get_int() {
    let ctx = test_config();
    ctx.config.set("test_int", 42i64);
    assert_eq!(
        ctx.config.get("test_int").and_then(|v| v.as_i64()),
        Some(42)
    );
}

#[test]
fn set_overwrites() {
    let ctx = test_config();
    ctx.config.set("key", "first");
    ctx.config.set("key", "second");
    assert_eq!(
        ctx.config
            .get("key")
            .and_then(|v| v.as_str().map(String::from)),
        Some("second".into())
    );
}

#[test]
fn remove_deletes_key_from_memory_and_config_file() {
    let ctx = test_config();
    ctx.config
        .set("openai@demo", serde_json::json!({"instanceName": "AI"}));

    ctx.config.remove("openai@demo");

    assert!(ctx.config.get("openai@demo").is_none());

    let config_file = ctx.config.config_dir().join("config.json");
    let stored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(config_file).unwrap()).unwrap();
    assert!(stored.get("openai@demo").is_none());
}

#[test]
fn config_dir_contains_app_id() {
    let ctx = test_config();
    let dir = ctx.config.config_dir();
    assert!(dir.to_string_lossy().contains("com.pot-app.desktop"));
}

#[test]
fn generate_instance_key_format() {
    let key = AppConfig::generate_instance_key("google");
    assert!(key.starts_with("google@"));
    assert!(key.len() > "google@".len());
}

#[test]
fn generate_instance_key_unique() {
    let k1 = AppConfig::generate_instance_key("bing");
    let k2 = AppConfig::generate_instance_key("bing");
    assert_ne!(k1, k2);
}

#[test]
fn effective_translate_service_list_includes_google_and_bing_by_default() {
    let ctx = test_config();

    assert_eq!(
        ctx.config
            .effective_service_list(ServiceCategory::Translate),
        vec!["google".to_string(), "bing".to_string()]
    );
}

#[test]
fn effective_translate_service_list_does_not_duplicate_locked_instance_service() {
    let ctx = test_config();
    ctx.config
        .set_service_list(ServiceCategory::Translate, &["bing@custom".to_string()]);

    assert_eq!(
        ctx.config
            .effective_service_list(ServiceCategory::Translate),
        vec!["bing@custom".to_string(), "google".to_string()]
    );
}

#[test]
fn effective_non_translate_service_list_passes_through_unchanged() {
    let ctx = test_config();
    ctx.config
        .set_service_list(ServiceCategory::Recognize, &["tesseract@local".to_string()]);

    assert_eq!(
        ctx.config
            .effective_service_list(ServiceCategory::Recognize),
        vec!["tesseract@local".to_string()]
    );
}

#[test]
fn service_category_list_key() {
    assert_eq!(
        ServiceCategory::Translate.list_key(),
        "translate_service_list"
    );
    assert_eq!(
        ServiceCategory::Recognize.list_key(),
        "recognize_service_list"
    );
    assert_eq!(ServiceCategory::Tts.list_key(), "tts_service_list");
    assert_eq!(
        ServiceCategory::Collection.list_key(),
        "collection_service_list"
    );
}
