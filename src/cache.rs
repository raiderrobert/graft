use std::path::PathBuf;

use crate::error::{GraftError, Result};

pub struct Cache {
    root: PathBuf,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    pub fn new() -> Self {
        let root = if let Ok(dir) = std::env::var("GRAFT_CACHE_DIR") {
            PathBuf::from(dir)
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".graft")
                .join("cache")
        };
        Self { root }
    }

    #[cfg(test)]
    pub fn with_root(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn key_path(&self, owner: &str, repo: &str, commit: &str, path: &str) -> PathBuf {
        self.root.join(owner).join(repo).join(commit).join(path)
    }

    pub fn get(&self, owner: &str, repo: &str, commit: &str, path: &str) -> Option<Vec<u8>> {
        std::fs::read(self.key_path(owner, repo, commit, path)).ok()
    }

    pub fn put(
        &self,
        owner: &str,
        repo: &str,
        commit: &str,
        path: &str,
        content: &[u8],
    ) -> Result<()> {
        let key = self.key_path(owner, repo, commit, path);
        if let Some(parent) = key.parent() {
            std::fs::create_dir_all(parent).map_err(|e| GraftError::Io {
                context: format!("creating cache directory {}", parent.display()),
                source: e,
            })?;
        }
        let mut tmp =
            tempfile::NamedTempFile::new_in(key.parent().unwrap()).map_err(|e| GraftError::Io {
                context: "creating temp file for cache".into(),
                source: e,
            })?;
        std::io::Write::write_all(&mut tmp, content).map_err(|e| GraftError::Io {
            context: "writing cache content".into(),
            source: e,
        })?;
        tmp.persist(&key).map_err(|e| GraftError::Io {
            context: format!("persisting cache file {}", key.display()),
            source: e.error,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_miss_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        assert!(cache.get("owner", "repo", "abc123", "file.txt").is_none());
    }

    #[test]
    fn put_then_get_returns_content() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let content = b"hello world";
        cache
            .put("owner", "repo", "abc123", "file.txt", content)
            .unwrap();
        let result = cache.get("owner", "repo", "abc123", "file.txt");
        assert_eq!(result, Some(content.to_vec()));
    }

    #[test]
    fn nested_path_works() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let content = b"name: lint\non: push";
        cache
            .put("owner", "repo", "def456", "workflows/lint.yml", content)
            .unwrap();
        let result = cache.get("owner", "repo", "def456", "workflows/lint.yml");
        assert_eq!(result, Some(content.to_vec()));
    }

    #[test]
    fn different_commits_return_different_content() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        cache
            .put("owner", "repo", "commit1", "file.txt", b"version 1")
            .unwrap();
        cache
            .put("owner", "repo", "commit2", "file.txt", b"version 2")
            .unwrap();
        assert_eq!(
            cache.get("owner", "repo", "commit1", "file.txt"),
            Some(b"version 1".to_vec())
        );
        assert_eq!(
            cache.get("owner", "repo", "commit2", "file.txt"),
            Some(b"version 2".to_vec())
        );
    }
}
