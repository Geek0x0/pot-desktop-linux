use crate::error::AppError;
use base64::{engine::general_purpose, Engine as _};
use std::collections::HashMap;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone)]
pub struct RecognizeRequest {
    pub image_base64: String,
    pub language: String,
    pub config: serde_json::Value,
}

#[derive(Debug)]
pub struct RecognizeResult {
    pub text: String,
}

pub trait RecognizeService: Send + Sync {
    fn name(&self) -> &str;
    fn recognize(&self, request: RecognizeRequest) -> Result<RecognizeResult>;
    fn default_config(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

pub struct RecognizeRegistry {
    services: HashMap<String, Box<dyn RecognizeService>>,
}

impl RecognizeRegistry {
    pub fn new() -> Self {
        RecognizeRegistry {
            services: HashMap::new(),
        }
    }

    pub fn register(&mut self, service: Box<dyn RecognizeService>) {
        self.services.insert(service.name().to_string(), service);
    }

    pub fn get(&self, name: &str) -> Option<&dyn RecognizeService> {
        self.services.get(name).map(|s| s.as_ref())
    }

    pub fn recognize(&self, name: &str, request: RecognizeRequest) -> Result<RecognizeResult> {
        let service = self
            .services
            .get(name)
            .ok_or_else(|| AppError::Custom(format!("Unknown OCR service: {}", name)))?;
        service.recognize(request)
    }

    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.services.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }
}

pub fn create_registry() -> RecognizeRegistry {
    let mut registry = RecognizeRegistry::new();
    registry.register(Box::new(SystemTesseract));
    registry
}

/// System tesseract OCR backend
struct SystemTesseract;

impl RecognizeService for SystemTesseract {
    fn name(&self) -> &str {
        "tesseract"
    }

    fn recognize(&self, request: RecognizeRequest) -> Result<RecognizeResult> {
        // Write base64 image to a temp file for tesseract
        let data = general_purpose::STANDARD
            .decode(request.image_base64)
            .map_err(|e| AppError::Custom(format!("Base64 decode failed: {}", e)))?;

        let temp_dir = std::env::temp_dir().join("pot-gtk-ocr");
        std::fs::create_dir_all(&temp_dir)?;
        let img_path = temp_dir.join(format!("ocr_input_{}.png", crate::util::nanoid(8)));
        std::fs::write(&img_path, &data)?;

        let mut cmd = std::process::Command::new("tesseract");
        cmd.arg(&img_path).arg("stdout");
        if request.language != "auto" && !request.language.is_empty() {
            cmd.arg("-l").arg(&request.language);
        }

        let output = cmd
            .output()
            .map_err(|e| AppError::Custom(format!("Failed to run tesseract: {}", e)))?;

        // Clean up temp file
        let _ = std::fs::remove_file(&img_path);

        if output.status.success() {
            Ok(RecognizeResult {
                text: String::from_utf8_lossy(&output.stdout).to_string(),
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(AppError::Custom(format!("Tesseract failed: {}", stderr)))
        }
    }
}
