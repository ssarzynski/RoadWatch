//! RoadWatch untrusted image ingestion.
//! Originals are hostile input. Only freshly decoded/re-encoded derivatives
//! may be considered for public evidence.

use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use thiserror::Error;

pub const MAX_UPLOAD_BYTES: usize = 12 * 1024 * 1024;
pub const MAX_WIDTH: u32 = 8_192;
pub const MAX_HEIGHT: u32 = 8_192;
pub const MAX_PIXELS: u64 = 40_000_000;

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("upload exceeds byte limit")]
    TooLarge,
    #[error("unsupported image format")]
    UnsupportedFormat,
    #[error("image dimensions exceed safety limits")]
    DimensionsExceeded,
    #[error("image decode failed")]
    DecodeFailed,
    #[error("sanitized image encode failed")]
    EncodeFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptedFormat {
    Jpeg,
    Png,
}

#[derive(Debug)]
pub struct SanitizedImage {
    pub format: AcceptedFormat,
    pub width: u32,
    pub height: u32,
    pub original_sha256: String,
    pub sanitized_sha256: String,
    /// 64-bit average-hash of decoded pixels for near-duplicate/replay screening.
    pub perceptual_hash: u64,
    /// Freshly encoded bytes. These are the only bytes eligible for public use.
    pub sanitized_bytes: Vec<u8>,
    /// Advisory only. Never means "steganography absent."
    pub anomaly_flags: Vec<&'static str>,
}

pub fn ingest(input: &[u8]) -> Result<SanitizedImage, IngestError> {
    if input.len() > MAX_UPLOAD_BYTES {
        return Err(IngestError::TooLarge);
    }

    let original_sha256 = sha256_hex(input);
    let format = detect_format(input)?;

    let reader = ImageReader::new(Cursor::new(input))
        .with_guessed_format()
        .map_err(|_| IngestError::DecodeFailed)?;
    let image = reader.decode().map_err(|_| IngestError::DecodeFailed)?;
    validate_dimensions(&image)?;

    let mut anomaly_flags = advisory_anomaly_flags(&image);
    if has_trailing_payload(input, format) {
        anomaly_flags.push("trailing_payload");
    }
    let perceptual_hash = average_hash(&image);

    // Re-encode decoded pixels into a fresh container. We intentionally do not
    // copy EXIF/XMP/IPTC/application segments or other original container data.
    let output_format = match format {
        AcceptedFormat::Jpeg => ImageFormat::Jpeg,
        AcceptedFormat::Png => ImageFormat::Png,
    };
    let mut sanitized_bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut sanitized_bytes), output_format)
        .map_err(|_| IngestError::EncodeFailed)?;

    let sanitized_sha256 = sha256_hex(&sanitized_bytes);

    Ok(SanitizedImage {
        format,
        width: image.width(),
        height: image.height(),
        original_sha256,
        sanitized_sha256,
        perceptual_hash,
        sanitized_bytes,
        anomaly_flags,
    })
}

fn detect_format(input: &[u8]) -> Result<AcceptedFormat, IngestError> {
    match image::guess_format(input).map_err(|_| IngestError::UnsupportedFormat)? {
        ImageFormat::Jpeg => Ok(AcceptedFormat::Jpeg),
        ImageFormat::Png => Ok(AcceptedFormat::Png),
        _ => Err(IngestError::UnsupportedFormat),
    }
}

fn validate_dimensions(image: &DynamicImage) -> Result<(), IngestError> {
    let (width, height) = image.dimensions();
    let pixels = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || width > MAX_WIDTH
        || height > MAX_HEIGHT
        || pixels > MAX_PIXELS
    {
        return Err(IngestError::DimensionsExceeded);
    }
    Ok(())
}

/// Lightweight anomaly indicators only. These cannot establish whether hidden
/// information is present or absent. Future sandboxed workers can add stronger
/// statistical analysis without weakening the decode/re-encode boundary.
fn advisory_anomaly_flags(image: &DynamicImage) -> Vec<&'static str> {
    let (w, h) = image.dimensions();
    let mut flags = Vec::new();
    if u64::from(w) * u64::from(h) > 30_000_000 {
        flags.push("very_large_pixel_count");
    }
    flags
}

/// Simple deterministic average hash (aHash) for replay/near-duplicate screening.
/// This is not a cryptographic hash and is never used for integrity decisions.
pub fn average_hash(image: &DynamicImage) -> u64 {
    let gray = image.resize_exact(8, 8, image::imageops::FilterType::Triangle).to_luma8();
    let sum: u32 = gray.pixels().map(|p| u32::from(p[0])).sum();
    let mean = sum / 64;
    gray.pixels().enumerate().fold(0_u64, |hash, (i, p)| {
        if u32::from(p[0]) >= mean { hash | (1_u64 << i) } else { hash }
    })
}

pub fn perceptual_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Detect obvious data appended after the normal image terminator.
/// A flag means quarantine/review; absence does not prove a file is benign.
pub fn has_trailing_payload(input: &[u8], format: AcceptedFormat) -> bool {
    match format {
        AcceptedFormat::Png => {
            // PNG IEND chunk is: length=0, type=IEND, CRC=AE426082.
            const IEND: &[u8] = &[0,0,0,0,b'I',b'E',b'N',b'D',0xAE,0x42,0x60,0x82];
            input.windows(IEND.len()).rposition(|w| w == IEND)
                .is_some_and(|pos| pos + IEND.len() != input.len())
        }
        AcceptedFormat::Jpeg => {
            // JPEG terminates with EOI FF D9. Bytes after the last EOI are suspicious.
            input.windows(2).rposition(|w| w == [0xFF, 0xD9])
                .is_some_and(|pos| pos + 2 != input.len())
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn tiny_png() -> Vec<u8> {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(2, 2, Rgb([1, 2, 3]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(img)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn rejects_non_image_bytes() {
        assert!(matches!(ingest(b"not an image"), Err(IngestError::UnsupportedFormat)));
    }

    #[test]
    fn accepts_and_reencodes_png() {
        let input = tiny_png();
        let result = ingest(&input).unwrap();
        assert_eq!(result.format, AcceptedFormat::Png);
        assert_eq!((result.width, result.height), (2, 2));
        assert!(!result.sanitized_bytes.is_empty());
    }

    #[test]
    fn hashes_are_sha256_hex() {
        let result = ingest(&tiny_png()).unwrap();
        assert_eq!(result.original_sha256.len(), 64);
        assert_eq!(result.sanitized_sha256.len(), 64);
    }

    #[test]
    fn perceptual_hash_is_stable_for_same_pixels() {
        let a = ingest(&tiny_png()).unwrap();
        let b = ingest(&tiny_png()).unwrap();
        assert_eq!(a.perceptual_hash, b.perceptual_hash);
        assert_eq!(perceptual_distance(a.perceptual_hash, b.perceptual_hash), 0);
    }

    #[test]
    fn appended_png_bytes_are_flagged() {
        let mut input = tiny_png();
        input.extend_from_slice(b"HIDDEN-PAYLOAD");
        let result = ingest(&input).unwrap();
        assert!(result.anomaly_flags.contains(&"trailing_payload"));
    }

    #[test]
    fn byte_limit_is_enforced_before_decode() {
        let input = vec![0_u8; MAX_UPLOAD_BYTES + 1];
        assert!(matches!(ingest(&input), Err(IngestError::TooLarge)));
    }
}
