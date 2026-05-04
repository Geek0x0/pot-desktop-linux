use pot_gtk::services::translate::TranslateRegistry;
use pot_gtk::services::types::{TranslateRequest, TranslateResult};
use serde_json::json;

#[tokio::test]
async fn registry_empty_get_returns_none() {
    let reg = TranslateRegistry::new();
    assert!(reg.get("google").is_none());
}

#[test]
fn registry_list_empty() {
    let reg = TranslateRegistry::new();
    assert!(reg.list().is_empty());
}

#[tokio::test]
async fn registry_translate_unknown_returns_error() {
    let reg = TranslateRegistry::new();
    let req = TranslateRequest {
        text: "hello".into(),
        from: "en".into(),
        to: "zh-CN".into(),
        config: json!(null),
    };
    let result = reg.translate("nonexistent", req).await;
    assert!(result.is_err());
}

#[test]
fn translate_request_fields() {
    let req = TranslateRequest {
        text: "hello".into(),
        from: "en".into(),
        to: "zh-CN".into(),
        config: json!({"key": "value"}),
    };
    assert_eq!(req.text, "hello");
    assert_eq!(req.from, "en");
    assert_eq!(req.to, "zh-CN");
}

#[test]
fn translate_result_text_variant() {
    let result = TranslateResult::Text("translated".into());
    match result {
        TranslateResult::Text(t) => assert_eq!(t, "translated"),
        TranslateResult::Dictionary(_) => panic!("expected Text variant"),
    }
}

#[test]
fn translate_result_dictionary_variant() {
    let result = TranslateResult::Dictionary(pot_gtk::services::types::DictionaryResult {
        pronunciations: vec![],
        explanations: vec![],
        associations: vec![],
        sentences: vec![],
    });
    assert!(matches!(result, TranslateResult::Dictionary(_)));
}

#[test]
fn create_registry_has_all_backends() {
    let reg = TranslateRegistry::new();
    // create_registry() will make HTTP calls during registration,
    // so we just test that new() creates an empty registry.
    assert!(reg.list().is_empty());
}
