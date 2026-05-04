use crate::config::APP_ID;
use crate::error::{AppError, Result};
use arboard::ImageData;
use base64::{engine::general_purpose, Engine as _};
use image::ImageReader;
use log::info;
use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;

fn cache_dir() -> Result<PathBuf> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| AppError::Custom("Failed to get cache directory".into()))?;
    Ok(dir.join(APP_ID))
}

fn screenshot_path() -> Result<PathBuf> {
    Ok(cache_dir()?.join("pot_screenshot.png"))
}

fn cut_path() -> Result<PathBuf> {
    Ok(cache_dir()?.join("pot_screenshot_cut.png"))
}

pub fn cut_image(left: u32, top: u32, width: u32, height: u32) -> Result<()> {
    info!("Cut image: {}x{}+{}+{}", width, height, left, top);
    let src = screenshot_path()?;
    if !src.exists() {
        return Err(AppError::Custom("Screenshot file not found".into()));
    }

    let img = image::open(&src)?;
    let cropped = img.crop_imm(left, top, width, height);
    let dst = cut_path()?;
    cropped.save(&dst)?;
    info!("Cut image saved to {:?}", dst);
    Ok(())
}

pub fn cut_image_scaled(
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    view_width: i32,
    view_height: i32,
    image_width: u32,
    image_height: u32,
) -> Result<()> {
    if view_width <= 0 || view_height <= 0 || image_width == 0 || image_height == 0 {
        return Err(AppError::Custom("Invalid screenshot dimensions".into()));
    }

    let scale_x = image_width as f64 / view_width as f64;
    let scale_y = image_height as f64 / view_height as f64;

    let x = (left * scale_x).round().clamp(0.0, image_width as f64) as u32;
    let y = (top * scale_y).round().clamp(0.0, image_height as f64) as u32;
    let right = ((left + width) * scale_x)
        .round()
        .clamp(0.0, image_width as f64) as u32;
    let bottom = ((top + height) * scale_y)
        .round()
        .clamp(0.0, image_height as f64) as u32;

    let crop_width = right.saturating_sub(x);
    let crop_height = bottom.saturating_sub(y);
    if crop_width == 0 || crop_height == 0 {
        return Err(AppError::Custom("Empty screenshot selection".into()));
    }

    cut_image(x, y, crop_width, crop_height)
}

pub fn get_base64() -> Result<String> {
    let path = cut_path()?;
    if !path.exists() {
        return Ok(String::new());
    }

    let data = fs::read(&path)?;
    Ok(general_purpose::STANDARD.encode(&data))
}

#[allow(dead_code)]
pub fn copy_img() -> Result<()> {
    let path = cut_path()?;
    if !path.exists() {
        return Err(AppError::Custom("Cut image file not found".into()));
    }

    let data = ImageReader::open(&path)?.decode()?;
    let img = ImageData {
        width: data.width() as usize,
        height: data.height() as usize,
        bytes: Cow::from(data.as_bytes()),
    };
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_image(img)?;
    Ok(())
}
