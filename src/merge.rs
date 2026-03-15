use std::process::Command;

use crate::error::{GraftError, Result};

#[derive(Debug, PartialEq)]
pub enum MergeResult {
    Clean(Vec<u8>),
    Conflict(Vec<u8>),
}

/// Three-way merge using `git merge-file -p`.
/// base = original upstream, ours = current local, theirs = new upstream.
/// Uses -p flag to print to stdout instead of modifying ours in place.
pub fn three_way_merge(base: &[u8], ours: &[u8], theirs: &[u8]) -> Result<MergeResult> {
    let dir = tempfile::tempdir().map_err(|e| GraftError::Io {
        context: "creating temp dir for merge".into(),
        source: e,
    })?;

    let base_path = dir.path().join("base");
    let ours_path = dir.path().join("ours");
    let theirs_path = dir.path().join("theirs");

    std::fs::write(&base_path, base).map_err(|e| GraftError::Io {
        context: "writing base".into(),
        source: e,
    })?;
    std::fs::write(&ours_path, ours).map_err(|e| GraftError::Io {
        context: "writing ours".into(),
        source: e,
    })?;
    std::fs::write(&theirs_path, theirs).map_err(|e| GraftError::Io {
        context: "writing theirs".into(),
        source: e,
    })?;

    let output = Command::new("git")
        .args(["merge-file", "-p"])
        .arg(&ours_path)
        .arg(&base_path)
        .arg(&theirs_path)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                GraftError::GitNotFound
            } else {
                GraftError::Io {
                    context: "running git merge-file".into(),
                    source: e,
                }
            }
        })?;

    if output.status.success() {
        Ok(MergeResult::Clean(output.stdout))
    } else if output.status.code() == Some(1) {
        Ok(MergeResult::Conflict(output.stdout))
    } else {
        Err(GraftError::MergeFailed {
            reason: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_merge_identical() {
        let content = b"line1\nline2\nline3\n";
        let result = three_way_merge(content, content, content).unwrap();
        assert_eq!(result, MergeResult::Clean(content.to_vec()));
    }

    #[test]
    fn clean_merge_non_overlapping_changes() {
        let base = b"line1\nline2\nline3\n";
        let ours = b"line1 modified\nline2\nline3\n";
        let theirs = b"line1\nline2\nline3 modified\n";
        let result = three_way_merge(base, ours, theirs).unwrap();
        match result {
            MergeResult::Clean(content) => {
                let text = String::from_utf8(content).unwrap();
                assert!(text.contains("line1 modified"));
                assert!(text.contains("line3 modified"));
            }
            MergeResult::Conflict(_) => panic!("expected clean merge"),
        }
    }

    #[test]
    fn conflict_overlapping_changes() {
        let base = b"line1\nline2\nline3\n";
        let ours = b"line1\nours change\nline3\n";
        let theirs = b"line1\ntheirs change\nline3\n";
        let result = three_way_merge(base, ours, theirs).unwrap();
        match result {
            MergeResult::Conflict(content) => {
                let text = String::from_utf8(content).unwrap();
                assert!(text.contains("<<<<<<<"));
                assert!(text.contains(">>>>>>>"));
                assert!(text.contains("ours change"));
                assert!(text.contains("theirs change"));
            }
            MergeResult::Clean(_) => panic!("expected conflict"),
        }
    }

    #[test]
    fn theirs_only_change() {
        let base = b"line1\nline2\nline3\n";
        let ours = base;
        let theirs = b"line1\nnew line2\nline3\n";
        let result = three_way_merge(base, ours, theirs).unwrap();
        assert_eq!(result, MergeResult::Clean(theirs.to_vec()));
    }

    #[test]
    fn ours_only_change() {
        let base = b"line1\nline2\nline3\n";
        let ours = b"line1\nour line2\nline3\n";
        let theirs = base;
        let result = three_way_merge(base, ours, theirs).unwrap();
        assert_eq!(result, MergeResult::Clean(ours.to_vec()));
    }
}
