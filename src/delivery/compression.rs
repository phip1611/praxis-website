// SPDX-License-Identifier: MIT OR Apache-2.0

//! Compression with the strongest settings.
//!
//! Compression runs once per content change and not per request, so its speed
//! doesn't matter. Only the size does.

use brotli::enc::BrotliEncoderParams;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::io::Write;

/// The highest Brotli quality.
const BROTLI_QUALITY: i32 = 11;

/// The Brotli window size as base-2 logarithm: 4 MiB, larger than any response.
const BROTLI_WINDOW_BITS: i32 = 22;

/// Compresses `data` with Brotli.
pub(super) fn brotli(data: &[u8]) -> Vec<u8> {
    let params = BrotliEncoderParams {
        quality: BROTLI_QUALITY,
        lgwin: BROTLI_WINDOW_BITS,
        ..BrotliEncoderParams::default()
    };
    let mut output = Vec::new();
    brotli::BrotliCompress(&mut &data[..], &mut output, &params)
        .expect("compressing into memory should succeed");
    output
}

/// Compresses `data` with gzip.
pub(super) fn gzip(data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(data)
        .expect("compressing into memory should succeed");
    encoder
        .finish()
        .expect("compressing into memory should succeed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use std::io::Read;

    /// Repetitive text, as HTML is.
    fn sample() -> Vec<u8> {
        "<p>Öffnungszeiten</p>\n".repeat(200).into_bytes()
    }

    /// Brotli output decompresses to the input and is much smaller.
    #[test]
    fn brotli_round_trips() {
        let compressed = brotli(&sample());
        let mut decompressed = Vec::new();
        brotli::BrotliDecompress(&mut &compressed[..], &mut decompressed)
            .expect("output should be valid Brotli");

        assert_eq!(decompressed, sample());
        assert!(compressed.len() < sample().len() / 10);
    }

    /// Gzip output decompresses to the input and is much smaller.
    #[test]
    fn gzip_round_trips() {
        let compressed = gzip(&sample());
        let mut decompressed = Vec::new();
        GzDecoder::new(&compressed[..])
            .read_to_end(&mut decompressed)
            .expect("output should be valid gzip");

        assert_eq!(decompressed, sample());
        assert!(compressed.len() < sample().len() / 10);
    }

    /// The same input gives the same output, so ETags stay stable across
    /// restarts.
    #[test]
    fn output_is_deterministic() {
        assert_eq!(gzip(&sample()), gzip(&sample()));
        assert_eq!(brotli(&sample()), brotli(&sample()));
    }
}
