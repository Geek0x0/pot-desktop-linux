use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Custom(String),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    StripPrefix(#[from] std::path::StripPrefixError),
    #[error(transparent)]
    Arboard(#[from] arboard::Error),
    #[error(transparent)]
    Image(#[from] image::ImageError),
    #[error(transparent)]
    FontKit(#[from] font_kit::error::SelectionError),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Rusqlite(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_error_message() {
        let err = AppError::Custom("test error".into());
        assert!(format!("{}", err).contains("test error"));
    }

    #[test]
    fn io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let app_err: AppError = io_err.into();
        assert!(format!("{}", app_err).contains("file missing"));
    }
}
