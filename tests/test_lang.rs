use pot_gtk::lang::Language;
use std::str::FromStr;

#[test]
fn parse_common_codes() {
    assert!(matches!(Language::from_str("en"), Ok(Language::En)));
    assert!(matches!(Language::from_str("zh-CN"), Ok(Language::ZhCn)));
    assert!(matches!(Language::from_str("zh-TW"), Ok(Language::ZhTw)));
    assert!(matches!(Language::from_str("ja"), Ok(Language::Ja)));
    assert!(matches!(Language::from_str("ko"), Ok(Language::Ko)));
    assert!(matches!(Language::from_str("fr"), Ok(Language::Fr)));
    assert!(matches!(Language::from_str("de"), Ok(Language::De)));
}

#[test]
fn parse_unknown_returns_err() {
    assert!(Language::from_str("xx").is_err());
}

#[test]
fn roundtrip_code() {
    for lang in Language::all() {
        if matches!(lang, Language::Auto) {
            continue; // Auto has no roundtrip code
        }
        let code = lang.code();
        assert!(
            Language::from_str(code).is_ok(),
            "roundtrip failed for {:?}, code={}",
            lang,
            code
        );
    }
}

#[test]
fn all_has_expected_count() {
    // 30 variants (Auto + 29 languages)
    assert_eq!(Language::all().len(), 30);
}

#[test]
fn display_names_are_non_empty() {
    for lang in Language::all() {
        assert!(
            !lang.display_name().is_empty(),
            "empty display name for {:?}",
            lang
        );
    }
}

#[test]
fn codes_are_unique() {
    let codes: Vec<&str> = Language::all().iter().map(|l| l.code()).collect();
    let mut seen = std::collections::HashSet::new();
    for code in &codes {
        assert!(seen.insert(*code), "duplicate code: {}", code);
    }
}
