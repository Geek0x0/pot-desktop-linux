#[cfg(feature = "ocr")]
use std::sync::LazyLock;

#[cfg(feature = "ocr")]
static DETECTOR: LazyLock<lingua::LanguageDetector> = LazyLock::new(|| {
    use lingua::{Language, LanguageDetectorBuilder};
    let languages = vec![
        Language::Chinese,
        Language::Japanese,
        Language::English,
        Language::Korean,
        Language::French,
        Language::Spanish,
        Language::German,
        Language::Russian,
        Language::Italian,
        Language::Portuguese,
        Language::Turkish,
        Language::Arabic,
        Language::Vietnamese,
        Language::Thai,
        Language::Indonesian,
        Language::Malay,
        Language::Hindi,
        Language::Mongolian,
        Language::Bokmal,
        Language::Nynorsk,
        Language::Persian,
        Language::Ukrainian,
    ];
    LanguageDetectorBuilder::from_languages(&languages).build()
});

#[cfg(feature = "ocr")]
pub fn detect(text: &str) -> Option<&'static str> {
    use lingua::Language;
    let lang = DETECTOR.detect_language_of(text)?;

    Some(match lang {
        Language::Chinese => "zh_cn",
        Language::Japanese => "ja",
        Language::English => "en",
        Language::Korean => "ko",
        Language::French => "fr",
        Language::Spanish => "es",
        Language::German => "de",
        Language::Russian => "ru",
        Language::Italian => "it",
        Language::Portuguese => "pt",
        Language::Turkish => "tr",
        Language::Arabic => "ar",
        Language::Vietnamese => "vi",
        Language::Thai => "th",
        Language::Indonesian => "id",
        Language::Malay => "ms",
        Language::Hindi => "hi",
        Language::Mongolian => "mn",
        Language::Bokmal => "nb",
        Language::Nynorsk => "no",
        Language::Persian => "fa",
        Language::Ukrainian => "uk",
        _ => "en",
    })
}

#[cfg(not(feature = "ocr"))]
pub fn detect(_text: &str) -> Option<&'static str> {
    None
}
