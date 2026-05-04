use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateRequest {
    pub text: String,
    pub from: String,
    pub to: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranslateResult {
    Text(String),
    Dictionary(DictionaryResult),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryResult {
    pub pronunciations: Vec<Pronunciation>,
    pub explanations: Vec<Explanation>,
    pub associations: Vec<String>,
    pub sentences: Vec<Sentence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pronunciation {
    pub region: String,
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explanation {
    pub trait_name: String,
    pub explains: Vec<Explain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explain {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sentence {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecognizeRequest {
    pub image: Vec<u8>,
    pub language: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
#[error("[{service}] {message}")]
pub struct ServiceError {
    pub service: String,
    pub message: String,
}
