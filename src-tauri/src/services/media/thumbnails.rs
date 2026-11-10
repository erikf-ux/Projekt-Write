//! Thumbnails (section 17: "Galerie, virtualisiert, mit Vorschaubildern aus
//! `thumbnails/`"; ADR 0019, decision 7) and the size limit against image
//! bombs (section 17: "Grenzwerte schützen vor Bildbomben, Startwert 100
//! Megapixel").
//!
//! Raster images are decoded in Rust with limits; the thumbnail keeps the
//! aspect ratio, at most [`THUMBNAIL_EDGE`] pixels on its longer side, as PNG
//! if the image has transparency and as JPEG otherwise. PDF thumbnails are
//! drawn by the PDF view and only checked here.

use std::io::Cursor;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};

/// Longer side of an image thumbnail.
pub const THUMBNAIL_EDGE: u32 = 320;
/// Longer side a PDF thumbnail from the interface may have.
pub const MAX_PDF_THUMBNAIL_EDGE: u32 = 512;
/// Largest PDF thumbnail accepted (bytes).
pub const MAX_PDF_THUMBNAIL_BYTES: usize = 4 * 1024 * 1024;
const JPEG_QUALITY: u8 = 80;

#[derive(Debug, thiserror::Error)]
pub enum ThumbnailError {
    #[error("image of {width} × {height} pixels is above {max_megapixels} megapixels")]
    TooManyPixels {
        width: u32,
        height: u32,
        max_megapixels: u32,
    },
    #[error("image cannot be decoded: {0}")]
    Decode(#[from] image::ImageError),
    #[error("read error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a valid thumbnail: {0}")]
    Invalid(&'static str),
}

/// File format of a thumbnail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailFormat {
    Png,
    Jpeg,
}

impl ThumbnailFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ThumbnailFormat::Png => "png",
            ThumbnailFormat::Jpeg => "jpg",
        }
    }

    pub fn mime_type(self) -> &'static str {
        match self {
            ThumbnailFormat::Png => "image/png",
            ThumbnailFormat::Jpeg => "image/jpeg",
        }
    }
}

/// Whether width × height stays within the limit.
pub fn within_megapixels(width: u32, height: u32, max_megapixels: u32) -> bool {
    u64::from(width) * u64::from(height) <= u64::from(max_megapixels) * 1_000_000
}

/// Width and height from the file header, without decoding the image.
pub fn dimensions(path: &Path) -> Result<(u32, u32), ThumbnailError> {
    Ok(ImageReader::open(path)?
        .with_guessed_format()?
        .into_dimensions()?)
}

/// Decodes `path` within the limit and makes its thumbnail.
pub fn render(
    path: &Path,
    max_megapixels: u32,
) -> Result<(Vec<u8>, ThumbnailFormat), ThumbnailError> {
    let (width, height) = dimensions(path)?;
    if !within_megapixels(width, height, max_megapixels) {
        return Err(ThumbnailError::TooManyPixels {
            width,
            height,
            max_megapixels,
        });
    }
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    let small = image.thumbnail(THUMBNAIL_EDGE, THUMBNAIL_EDGE);
    let mut bytes = Vec::new();
    if small.color().has_alpha() {
        small.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)?;
        Ok((bytes, ThumbnailFormat::Png))
    } else {
        let rgb = small.to_rgb8();
        JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY).encode_image(&rgb)?;
        Ok((bytes, ThumbnailFormat::Jpeg))
    }
}

/// Checks a PDF thumbnail drawn by the interface: a PNG that decodes and
/// is not larger than [`MAX_PDF_THUMBNAIL_EDGE`] on either side.
pub fn check_pdf_thumbnail(png: &[u8]) -> Result<(), ThumbnailError> {
    if png.len() > MAX_PDF_THUMBNAIL_BYTES {
        return Err(ThumbnailError::Invalid("too large"));
    }
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(ThumbnailError::Invalid("not a PNG"));
    }
    let reader = ImageReader::with_format(Cursor::new(png), ImageFormat::Png);
    let (width, height) = reader.into_dimensions()?;
    if width == 0
        || height == 0
        || width > MAX_PDF_THUMBNAIL_EDGE
        || height > MAX_PDF_THUMBNAIL_EDGE
    {
        return Err(ThumbnailError::Invalid("size"));
    }
    image::load_from_memory_with_format(png, ImageFormat::Png)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    fn temp_dir() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/tmp");
        std::fs::create_dir_all(&base).unwrap();
        tempfile::tempdir_in(base).unwrap()
    }

    #[test]
    fn a_thumbnail_keeps_the_aspect_ratio_and_the_transparency() {
        let dir = temp_dir();
        let opaque = dir.path().join("a.png");
        RgbImage::from_pixel(1000, 500, Rgb([200, 30, 30]))
            .save(&opaque)
            .unwrap();
        let (bytes, format) = render(&opaque, 100).unwrap();
        assert_eq!(format, ThumbnailFormat::Jpeg);
        let small = image::load_from_memory(&bytes).unwrap();
        assert_eq!((small.width(), small.height()), (320, 160));

        let clear = dir.path().join("b.png");
        RgbaImage::from_pixel(200, 400, Rgba([0, 0, 0, 0]))
            .save(&clear)
            .unwrap();
        let (bytes, format) = render(&clear, 100).unwrap();
        assert_eq!(format, ThumbnailFormat::Png);
        let small = image::load_from_memory(&bytes).unwrap();
        assert_eq!((small.width(), small.height()), (160, 320));
    }

    #[test]
    fn images_above_the_limit_are_not_decoded() {
        let dir = temp_dir();
        let path = dir.path().join("big.png");
        RgbImage::new(2000, 1000).save(&path).unwrap();
        assert!(within_megapixels(2000, 1000, 2));
        assert!(!within_megapixels(2001, 1000, 2));
        assert!(matches!(
            render(&path, 1),
            Err(ThumbnailError::TooManyPixels {
                width: 2000,
                height: 1000,
                ..
            })
        ));
    }

    #[test]
    fn a_pdf_thumbnail_must_be_a_small_png() {
        let mut png = Vec::new();
        RgbImage::new(300, 400)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        check_pdf_thumbnail(&png).unwrap();
        let mut big = Vec::new();
        RgbImage::new(600, 400)
            .write_to(&mut Cursor::new(&mut big), ImageFormat::Png)
            .unwrap();
        assert!(check_pdf_thumbnail(&big).is_err());
        assert!(check_pdf_thumbnail(b"GIF89a").is_err());
        let mut broken = png.clone();
        broken.truncate(60);
        assert!(check_pdf_thumbnail(&broken).is_err());
    }
}
