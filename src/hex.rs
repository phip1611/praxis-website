// SPDX-License-Identifier: MIT OR Apache-2.0

//! Lowercase hex encoding, for tokens, digests, and content hashes.

use std::fmt::Write;

/// Encodes `bytes` as lowercase hex.
pub(crate) fn encode(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(hex, "{byte:02x}").expect("writing to a string should succeed");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Leading zeros are kept, so the length is always twice the input.
    #[test]
    fn encodes_with_leading_zeros() {
        assert_eq!(encode(&[0x00, 0x0f, 0xab]), "000fab");
        assert!(encode(&[]).is_empty());
    }
}
