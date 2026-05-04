use crate::error::{AppError, Result};

#[allow(dead_code)]
pub fn list_fonts() -> Result<Vec<String>> {
    let source = font_kit::source::SystemSource::new();
    source
        .all_families()
        .map_err(|e| AppError::Custom(format!("Failed to list fonts: {}", e)))
}
