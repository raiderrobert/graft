use std::path::Path;

use crate::checksum::{checksum_directory_on_disk, checksum_file_on_disk};
use crate::config::lockfile::LockedDep;
use crate::config::manifest::GraftDep;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum GraftState {
    Synced,
    Modified,
    Outdated,
    Conflicted,
    Missing,
}

impl std::fmt::Display for GraftState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraftState::Synced => write!(f, "synced"),
            GraftState::Modified => write!(f, "modified"),
            GraftState::Outdated => write!(f, "outdated"),
            GraftState::Conflicted => write!(f, "conflicted"),
            GraftState::Missing => write!(f, "missing"),
        }
    }
}

pub fn compute_state(
    dep: &GraftDep,
    locked: Option<&LockedDep>,
    project_root: &Path,
) -> Result<GraftState> {
    let dest = project_root.join(&dep.dest);
    if !dest.exists() {
        return Ok(GraftState::Missing);
    }
    let Some(locked) = locked else {
        return Ok(GraftState::Missing);
    };
    // Check for conflict markers
    if let Ok(content) = std::fs::read_to_string(&dest) {
        if content.contains("<<<<<<<") && content.contains(">>>>>>>") {
            return Ok(GraftState::Conflicted);
        }
    }
    // Compare checksum
    let current_checksum = if dest.is_dir() {
        let files = locked.files.as_deref().unwrap_or(&[]);
        checksum_directory_on_disk(&dest, files)?
    } else {
        checksum_file_on_disk(&dest)?
    };
    if current_checksum == locked.checksum {
        Ok(GraftState::Synced)
    } else {
        Ok(GraftState::Modified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::checksum_bytes;

    fn make_dep(dest: &str) -> GraftDep {
        GraftDep {
            source: "gh:owner/repo/file".to_string(),
            version: "v1.0.0".to_string(),
            dest: dest.to_string(),
            files: None,
        }
    }

    fn make_locked(checksum: &str) -> LockedDep {
        LockedDep {
            source: "gh:owner/repo/file".to_string(),
            version: "v1.0.0".to_string(),
            commit: "abc123".to_string(),
            checksum: checksum.to_string(),
            files: None,
        }
    }

    #[test]
    fn missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let dep = make_dep("nonexistent.txt");
        let locked = make_locked("sha256:000");
        let state = compute_state(&dep, Some(&locked), dir.path()).unwrap();
        assert_eq!(state, GraftState::Missing);
    }

    #[test]
    fn missing_no_lockfile_entry() {
        let dir = tempfile::tempdir().unwrap();
        let dep = make_dep("nonexistent.txt");
        let state = compute_state(&dep, None, dir.path()).unwrap();
        assert_eq!(state, GraftState::Missing);
    }

    #[test]
    fn file_exists_but_no_lock_entry() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file.txt"), "hello").unwrap();
        let dep = make_dep("file.txt");
        let state = compute_state(&dep, None, dir.path()).unwrap();
        assert_eq!(state, GraftState::Missing);
    }

    #[test]
    fn synced_file() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"hello world";
        std::fs::write(dir.path().join("file.txt"), content).unwrap();
        let checksum = checksum_bytes(content);
        let dep = make_dep("file.txt");
        let locked = make_locked(&checksum);
        let state = compute_state(&dep, Some(&locked), dir.path()).unwrap();
        assert_eq!(state, GraftState::Synced);
    }

    #[test]
    fn modified_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file.txt"), "modified content").unwrap();
        let dep = make_dep("file.txt");
        let locked = make_locked(&checksum_bytes(b"original content"));
        let state = compute_state(&dep, Some(&locked), dir.path()).unwrap();
        assert_eq!(state, GraftState::Modified);
    }

    #[test]
    fn conflicted_file() {
        let dir = tempfile::tempdir().unwrap();
        let content = "line1\n<<<<<<< HEAD\nlocal\n=======\nremote\n>>>>>>> upstream\nline2\n";
        std::fs::write(dir.path().join("file.txt"), content).unwrap();
        let dep = make_dep("file.txt");
        let locked = make_locked(&checksum_bytes(b"something else"));
        let state = compute_state(&dep, Some(&locked), dir.path()).unwrap();
        assert_eq!(state, GraftState::Conflicted);
    }

    #[test]
    fn display_variants() {
        assert_eq!(GraftState::Synced.to_string(), "synced");
        assert_eq!(GraftState::Modified.to_string(), "modified");
        assert_eq!(GraftState::Outdated.to_string(), "outdated");
        assert_eq!(GraftState::Conflicted.to_string(), "conflicted");
        assert_eq!(GraftState::Missing.to_string(), "missing");
    }
}
