use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{GraftError, Result};

/// Compute SHA-256 checksum of raw bytes. Returns "sha256:{hex}".
pub fn checksum_bytes(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let hash = hasher.finalize();
    format!("sha256:{}", hex::encode(hash))
}

/// Compute deterministic checksum for a directory.
/// Sort files by path, compute each SHA-256, concatenate hex hashes, hash the result.
pub fn checksum_directory(files: &[(String, Vec<u8>)]) -> String {
    let mut sorted: Vec<_> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut combined = String::new();
    for (_, content) in &sorted {
        let mut hasher = Sha256::new();
        hasher.update(content);
        combined.push_str(&hex::encode(hasher.finalize()));
    }

    let mut hasher = Sha256::new();
    hasher.update(combined.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Compute checksum of a file on disk.
pub fn checksum_file_on_disk(path: &Path) -> Result<String> {
    let content = std::fs::read(path).map_err(|e| GraftError::Io {
        context: format!("reading file {}", path.display()),
        source: e,
    })?;
    Ok(checksum_bytes(&content))
}

/// Compute directory checksum from files on disk.
pub fn checksum_directory_on_disk(dir: &Path, files: &[String]) -> Result<String> {
    let mut file_contents = Vec::new();
    for file in files {
        let path = dir.join(file);
        let content = std::fs::read(&path).map_err(|e| GraftError::Io {
            context: format!("reading file {}", path.display()),
            source: e,
        })?;
        file_contents.push((file.clone(), content));
    }
    Ok(checksum_directory(&file_contents))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_bytes_returns_prefixed_hex() {
        let result = checksum_bytes(b"hello");
        assert!(result.starts_with("sha256:"));
        // "sha256:" is 7 chars, hex SHA-256 is 64 chars
        assert_eq!(result.len(), 7 + 64);
        // Verify all chars after prefix are hex
        assert!(result[7..].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn same_content_same_checksum() {
        let a = checksum_bytes(b"deterministic");
        let b = checksum_bytes(b"deterministic");
        assert_eq!(a, b);
    }

    #[test]
    fn different_content_different_checksum() {
        let a = checksum_bytes(b"hello");
        let b = checksum_bytes(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn directory_checksum_is_order_independent() {
        let files_ab = vec![
            ("a.txt".to_string(), b"alpha".to_vec()),
            ("b.txt".to_string(), b"beta".to_vec()),
        ];
        let files_ba = vec![
            ("b.txt".to_string(), b"beta".to_vec()),
            ("a.txt".to_string(), b"alpha".to_vec()),
        ];
        assert_eq!(checksum_directory(&files_ab), checksum_directory(&files_ba));
    }

    #[test]
    fn checksum_file_on_disk_matches_checksum_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let content = b"file content for checksum";
        std::fs::write(&file_path, content).unwrap();

        let from_disk = checksum_file_on_disk(&file_path).unwrap();
        let from_bytes = checksum_bytes(content);
        assert_eq!(from_disk, from_bytes);
    }
}
