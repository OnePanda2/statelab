//! Renders an avalanche matrix as an image (proposal §6.3 D4).
//!
//! This is not decoration. Triangular structure is obvious to the eye and
//! nearly invisible in summary statistics — `mean_deviation` can look
//! unremarkable while the matrix is visibly half empty. This project has
//! already had one defect that its test suite passed and a rendered image
//! caught, so visual inspection is part of the battery rather than a
//! nicety.
//!
//! Output is binary PPM (P6): no dependencies, and every image viewer and
//! conversion tool reads it.

use crate::avalanche::AvalancheMatrix;

/// Maps a probability to a diverging colour ramp centred on the ideal 0.5.
///
/// * 0.0 → blue   (never flips: a dead dependency)
/// * 0.5 → white  (ideal)
/// * 1.0 → red    (always flips: equally defective, and easy to miss)
fn colour(p: f64) -> [u8; 3] {
    let t = (p.clamp(0.0, 1.0) - 0.5) * 2.0; // -1..=1
    if t < 0.0 {
        let k = (1.0 + t).clamp(0.0, 1.0); // 0 at p=0, 1 at p=0.5
        let c = (k * 255.0).round() as u8;
        [c, c, 255]
    } else {
        let k = (1.0 - t).clamp(0.0, 1.0); // 1 at p=0.5, 0 at p=1
        let c = (k * 255.0).round() as u8;
        [255, c, c]
    }
}

/// Encodes the matrix as a binary PPM, one pixel per bit pair, scaled by
/// `scale` so small matrices are visible without a zoom tool.
///
/// Row = input bit, column = output bit, both increasing from the top-left.
pub fn matrix_to_ppm(m: &AvalancheMatrix, scale: usize) -> Vec<u8> {
    let scale = scale.max(1);
    let side = m.bits * scale;
    let mut out = Vec::with_capacity(side * side * 3 + 32);
    out.extend_from_slice(format!("P6\n{side} {side}\n255\n").as_bytes());

    for y in 0..side {
        let i = y / scale;
        for x in 0..side {
            let j = x / scale;
            out.extend_from_slice(&colour(m.get(i, j)));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// PNG
// ---------------------------------------------------------------------------
//
// PPM is dependency-free but not viewable on Windows without extra tooling, and
// an image nobody opens defeats the point of D4. This is a minimal PNG writer:
// truecolour, 8-bit, no filtering, and DEFLATE "stored" blocks so no compressor
// is needed. Files are larger than a real encoder would produce and that is a
// deliberate trade for having zero dependencies.

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Wraps raw bytes in a zlib stream of uncompressed DEFLATE blocks.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01]; // CMF/FLG; (0x78<<8|0x01) % 31 == 0
    if raw.is_empty() {
        z.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    }
    for (i, block) in raw.chunks(0xFFFF).enumerate() {
        let is_last = (i + 1) * 0xFFFF >= raw.len();
        z.push(u8::from(is_last));
        let len = block.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(raw).to_be_bytes());
    z
}

/// Encodes the matrix as a PNG, one pixel per bit pair, scaled by `scale`.
///
/// Row = input bit, column = output bit, matching [`matrix_to_ppm`].
pub fn matrix_to_png(m: &AvalancheMatrix, scale: usize) -> Vec<u8> {
    let scale = scale.max(1);
    let side = m.bits * scale;

    // Scanlines, each prefixed with filter type 0 (None).
    let mut raw = Vec::with_capacity(side * (1 + side * 3));
    for y in 0..side {
        raw.push(0);
        let i = y / scale;
        for x in 0..side {
            raw.extend_from_slice(&colour(m.get(i, x / scale)));
        }
    }

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(side as u32).to_be_bytes());
    ihdr.extend_from_slice(&(side as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, truecolour, no interlace
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &zlib_stored(&raw));
    chunk(&mut png, b"IEND", &[]);
    png
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matrix(bits: usize, p: Vec<f64>) -> AvalancheMatrix {
        AvalancheMatrix {
            bits,
            rounds: 1,
            samples: 1,
            p,
        }
    }

    #[test]
    fn ideal_probability_renders_white_and_dead_renders_blue() {
        assert_eq!(colour(0.5), [255, 255, 255]);
        assert_eq!(colour(0.0), [0, 0, 255]);
        assert_eq!(colour(1.0), [255, 0, 0]);
    }

    #[test]
    fn ppm_header_and_payload_have_the_expected_size() {
        let m = matrix(2, vec![0.5, 0.5, 0.5, 0.5]);
        let ppm = matrix_to_ppm(&m, 3);
        let side = 2 * 3;
        let header = format!("P6\n{side} {side}\n255\n");
        assert!(ppm.starts_with(header.as_bytes()));
        assert_eq!(ppm.len(), header.len() + side * side * 3);
    }

    /// CRC32 and Adler32 against published check values. Without these the
    /// encoder could emit a structurally plausible but corrupt file.
    #[test]
    fn checksums_match_published_values() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(adler32(b"123456789"), 0x091E_01DE);
        assert_eq!(adler32(b""), 1);
    }

    #[test]
    fn png_has_a_valid_signature_and_chunk_layout() {
        let m = matrix(4, vec![0.5; 16]);
        let png = matrix_to_png(&m, 2);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        // IHDR: length 13, then the type tag.
        assert_eq!(&png[8..12], &13u32.to_be_bytes());
        assert_eq!(&png[12..16], b"IHDR");
        let side = 4u32 * 2;
        assert_eq!(&png[16..20], &side.to_be_bytes());
        assert_eq!(&png[20..24], &side.to_be_bytes());
        assert_eq!(&png[24..29], &[8, 2, 0, 0, 0]);
        assert!(
            png.ends_with(b"IEND\xAE\x42\x60\x82"),
            "IEND chunk with its fixed CRC"
        );
    }

    #[test]
    fn zlib_stream_is_well_formed() {
        let z = zlib_stored(b"hello");
        assert_eq!(z[0], 0x78);
        assert_eq!((u16::from(z[0]) * 256 + u16::from(z[1])) % 31, 0);
        assert_eq!(z[2], 1, "single final stored block");
        assert_eq!(u16::from_le_bytes([z[3], z[4]]), 5);
        assert_eq!(u16::from_le_bytes([z[5], z[6]]), !5u16);
        assert_eq!(&z[7..12], b"hello");
        assert_eq!(&z[12..], &adler32(b"hello").to_be_bytes());
    }

    #[test]
    fn pixels_are_indexed_row_by_input_bit() {
        // Input bit 0 is dead everywhere; input bit 1 is ideal everywhere.
        let m = matrix(2, vec![0.0, 0.0, 0.5, 0.5]);
        let ppm = matrix_to_ppm(&m, 1);
        let body = &ppm[ppm.len() - 12..];
        assert_eq!(&body[0..3], &[0, 0, 255], "top-left must be the dead bit");
        assert_eq!(&body[6..9], &[255, 255, 255], "second row must be ideal");
    }
}
