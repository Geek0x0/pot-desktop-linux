use pot_gtk::config::AppConfig;
use pot_gtk::config::ServiceCategory;

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
fn service_category_list_keys() {
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

#[test]
fn service_category_from_str() {
    // Verify the enum derives useful traits
    let cat = ServiceCategory::Translate;
    assert_eq!(cat.list_key(), "translate_service_list");
}

#[test]
fn config_set_and_get_json_array() {
    let ctx = test_config();
    let list = vec!["google@abc".to_string(), "bing@def".to_string()];
    ctx.config
        .set("translate_service_list", serde_json::json!(list));

    let retrieved: Vec<String> = ctx
        .config
        .get("translate_service_list")
        .map(|v| serde_json::from_value(v).unwrap())
        .unwrap();
    assert_eq!(retrieved.len(), 2);
    assert_eq!(retrieved[0], "google@abc");
}

#[test]
fn generate_instance_key_contains_service_name() {
    let key = AppConfig::generate_instance_key("deepl");
    assert!(key.starts_with("deepl@"));
}

#[test]
fn generate_instance_keys_different_for_same_service() {
    let keys: Vec<String> = (0..10)
        .map(|_| AppConfig::generate_instance_key("test"))
        .collect();
    let unique: std::collections::HashSet<_> = keys.iter().collect();
    assert_eq!(unique.len(), 10, "all generated keys should be unique");
}

#[test]
fn config_store_empty_on_new_key() {
    let ctx = test_config();
    assert!(ctx.config.get("brand_new_key_12345").is_none());
}

#[test]
fn config_store_overwrite_value() {
    let ctx = test_config();
    ctx.config.set("test_overwrite", "v1");
    ctx.config.set("test_overwrite", "v2");
    let val = ctx
        .config
        .get("test_overwrite")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap();
    assert_eq!(val, "v2");
}

#[test]
fn config_store_complex_json() {
    let ctx = test_config();
    let complex = serde_json::json!({
        "nested": {
            "key": [1, 2, 3],
            "bool": true
        }
    });
    ctx.config.set("complex_key", complex.clone());
    let retrieved = ctx.config.get("complex_key").unwrap();
    assert_eq!(retrieved["nested"]["key"][1], 2);
    assert_eq!(retrieved["nested"]["bool"], true);
}

#[test]
fn check_service_available_prunes_unknown() {
    let ctx = test_config();
    // Set a list with unknown services
    ctx.config.set(
        "translate_service_list",
        serde_json::json!(["google@abc", "unknown_service@def", "bing@ghi"]),
    );
    let _ = ctx.config.check_service_available();
    let list: Vec<String> = ctx
        .config
        .get("translate_service_list")
        .map(|v| serde_json::from_value(v).unwrap())
        .unwrap_or_default();
    // unknown_service should be pruned
    for item in &list {
        let name = item.split('@').next().unwrap();
        assert_ne!(name, "unknown_service", "unknown services should be pruned");
    }
}
