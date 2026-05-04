use base64::{engine::general_purpose, Engine as _};

#[test]
fn base64_roundtrip() {
    let original = b"test image data";
    let encoded = general_purpose::STANDARD.encode(original);
    let decoded = general_purpose::STANDARD.decode(&encoded).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn base64_empty_input() {
    let encoded = general_purpose::STANDARD.encode(b"");
    let decoded = general_purpose::STANDARD.decode(&encoded).unwrap();
    assert!(decoded.is_empty());
}

#[test]
fn image_crop_produces_smaller_image() {
    let img = image::RgbImage::from_pixel(100, 100, image::Rgb([255, 255, 255]));
    let mut dynamic = image::DynamicImage::ImageRgb8(img);
    let cropped = dynamic.crop(10, 10, 50, 50);
    assert_eq!(cropped.width(), 50);
    assert_eq!(cropped.height(), 50);
}

#[test]
fn image_read_pixel() {
    let mut img = image::RgbImage::new(10, 10);
    img.put_pixel(5, 5, image::Rgb([255, 0, 0]));
    let pixel = img.get_pixel(5, 5);
    assert_eq!(pixel.0[0], 255);
    assert_eq!(pixel.0[1], 0);
}

#[test]
fn image_to_base64_and_back() {
    let img = image::RgbImage::from_pixel(2, 2, image::Rgb([128, 128, 128]));
    let mut png_buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut png_buf, image::ImageFormat::Png)
        .unwrap();
    let encoded = general_purpose::STANDARD.encode(png_buf.into_inner());

    // Decode back
    let decoded = general_purpose::STANDARD.decode(&encoded).unwrap();
    let recovered = image::load_from_memory(&decoded).unwrap();
    assert_eq!(recovered.width(), 2);
    assert_eq!(recovered.height(), 2);
}
