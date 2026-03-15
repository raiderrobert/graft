use std::collections::HashMap;

use semver::Version;

/// Parse `git ls-remote --tags` output into (tag_name, commit_sha) pairs.
/// Handles annotated tags: when a `^{}` entry exists, use its SHA and discard the non-dereferenced one.
pub fn parse_ls_remote_tags(output: &str) -> Vec<(String, String)> {
    let mut tag_map: HashMap<String, String> = HashMap::new();
    let mut seen_deref: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.splitn(2, '\t');
        let sha = match parts.next() {
            Some(s) => s.trim().to_string(),
            None => continue,
        };
        let refname = match parts.next() {
            Some(s) => s.trim(),
            None => continue,
        };

        let tag_name = match refname.strip_prefix("refs/tags/") {
            Some(name) => name.to_string(),
            None => continue,
        };

        if let Some(base) = tag_name.strip_suffix("^{}") {
            // Dereferenced entry — this is the commit SHA we want.
            tag_map.insert(base.to_string(), sha);
            seen_deref.insert(base.to_string());
        } else if !seen_deref.contains(&tag_name) {
            tag_map.insert(tag_name, sha);
        }
    }

    tag_map.into_iter().collect()
}

/// Strip leading 'v' or 'V' for semver parsing.
fn strip_v(tag: &str) -> &str {
    tag.strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag)
}

/// Parse a tag name as semver, stripping a leading 'v'.
fn parse_semver(tag: &str) -> Option<Version> {
    Version::parse(strip_v(tag)).ok()
}

/// Sort tags by semver (strip leading 'v' for parsing).
/// Non-semver tags sort after semver tags, lexicographically among themselves.
pub fn sort_tags(tags: &[(String, String)]) -> Vec<(String, String)> {
    let mut result = tags.to_vec();
    result.sort_by(|a, b| {
        let av = parse_semver(&a.0);
        let bv = parse_semver(&b.0);
        match (av, bv) {
            (Some(av), Some(bv)) => av.cmp(&bv),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.0.cmp(&b.0),
        }
    });
    result
}

/// Find tags newer than `current` from a sorted list.
pub fn find_newer_tags(current: &str, tags: &[(String, String)]) -> Vec<(String, String)> {
    if let Some(current_ver) = parse_semver(current) {
        // Find the position after the current version
        let pos = tags
            .iter()
            .position(|(tag, _)| parse_semver(tag).is_some_and(|v| v > current_ver));
        match pos {
            Some(idx) => tags[idx..].to_vec(),
            None => Vec::new(),
        }
    } else {
        // Non-semver: compare lexicographically
        let pos = tags.iter().position(|(tag, _)| tag.as_str() > current);
        match pos {
            Some(idx) => tags[idx..].to_vec(),
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_tags() {
        let output = "\
abc123\trefs/tags/v1.0.0
def456\trefs/tags/v2.0.0
";
        let tags = parse_ls_remote_tags(output);
        assert_eq!(tags.len(), 2);
        let map: HashMap<String, String> = tags.into_iter().collect();
        assert_eq!(map["v1.0.0"], "abc123");
        assert_eq!(map["v2.0.0"], "def456");
    }

    #[test]
    fn parse_annotated_tags_prefers_deref() {
        let output = "\
aaa111\trefs/tags/v1.0.0
bbb222\trefs/tags/v1.0.0^{}
ccc333\trefs/tags/v2.0.0
";
        let tags = parse_ls_remote_tags(output);
        let map: HashMap<String, String> = tags.into_iter().collect();
        // v1.0.0 should use the ^{} SHA
        assert_eq!(map["v1.0.0"], "bbb222");
        assert_eq!(map["v2.0.0"], "ccc333");
    }

    #[test]
    fn sort_semver_tags() {
        let tags = vec![
            ("v2.0.0".to_string(), "sha2".to_string()),
            ("v1.0.0".to_string(), "sha1".to_string()),
            ("v1.5.0".to_string(), "sha15".to_string()),
        ];
        let sorted = sort_tags(&tags);
        assert_eq!(sorted[0].0, "v1.0.0");
        assert_eq!(sorted[1].0, "v1.5.0");
        assert_eq!(sorted[2].0, "v2.0.0");
    }

    #[test]
    fn sort_mixed_semver_and_nonsemver() {
        let tags = vec![
            ("nightly".to_string(), "sha_n".to_string()),
            ("v2.0.0".to_string(), "sha2".to_string()),
            ("v1.0.0".to_string(), "sha1".to_string()),
            ("beta".to_string(), "sha_b".to_string()),
        ];
        let sorted = sort_tags(&tags);
        // Semver tags come first, sorted by version
        assert_eq!(sorted[0].0, "v1.0.0");
        assert_eq!(sorted[1].0, "v2.0.0");
        // Non-semver tags come after, sorted lexicographically
        assert_eq!(sorted[2].0, "beta");
        assert_eq!(sorted[3].0, "nightly");
    }

    #[test]
    fn find_newer_semver_tags() {
        let tags = vec![
            ("v1.0.0".to_string(), "sha1".to_string()),
            ("v1.5.0".to_string(), "sha15".to_string()),
            ("v2.0.0".to_string(), "sha2".to_string()),
            ("v3.0.0".to_string(), "sha3".to_string()),
        ];
        let newer = find_newer_tags("v1.5.0", &tags);
        assert_eq!(newer.len(), 2);
        assert_eq!(newer[0].0, "v2.0.0");
        assert_eq!(newer[1].0, "v3.0.0");
    }

    #[test]
    fn find_newer_tags_none_available() {
        let tags = vec![
            ("v1.0.0".to_string(), "sha1".to_string()),
            ("v2.0.0".to_string(), "sha2".to_string()),
        ];
        let newer = find_newer_tags("v2.0.0", &tags);
        assert!(newer.is_empty());
    }

    #[test]
    fn find_newer_tags_with_v_prefix_current() {
        let tags = vec![
            ("v1.0.0".to_string(), "sha1".to_string()),
            ("v2.0.0".to_string(), "sha2".to_string()),
        ];
        let newer = find_newer_tags("v1.0.0", &tags);
        assert_eq!(newer.len(), 1);
        assert_eq!(newer[0].0, "v2.0.0");
    }
}
