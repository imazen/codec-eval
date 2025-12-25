//! Checksum computation for image files.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::error::Result;

/// Compute a checksum for a file.
///
/// Uses a fast hash (FNV-1a) suitable for deduplication.
pub fn compute_checksum(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; 8192];

    // FNV-1a 64-bit hash
    let mut hash: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        for &byte in &buffer[..bytes_read] {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }

    Ok(format!("{hash:016x}"))
}

/// Compute a checksum for in-memory data.
#[must_use]
#[allow(dead_code)] // Used in tests, may be useful for API consumers
pub fn compute_checksum_bytes(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_bytes() {
        let data = b"hello world";
        let checksum = compute_checksum_bytes(data);
        assert_eq!(checksum.len(), 16);

        // Same data should produce same checksum
        let checksum2 = compute_checksum_bytes(data);
        assert_eq!(checksum, checksum2);

        // Different data should produce different checksum
        let checksum3 = compute_checksum_bytes(b"hello world!");
        assert_ne!(checksum, checksum3);
    }

    #[test]
    fn test_checksum_empty() {
        let checksum = compute_checksum_bytes(b"");
        assert_eq!(checksum.len(), 16);
    }
}
