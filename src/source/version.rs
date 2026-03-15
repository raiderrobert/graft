/// Represents whether a version string is a commit SHA or a tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionType {
    Tag(String),
    Sha(String),
}

/// Detect whether a version string is a SHA or a tag.
///
/// A SHA is defined as 7-64 lowercase hexadecimal characters.
/// Everything else is treated as a tag.
pub fn detect_version_type(version: &str) -> VersionType {
    let len = version.len();
    if (7..=64).contains(&len)
        && version
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
    {
        VersionType::Sha(version.to_string())
    } else {
        VersionType::Tag(version.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_sha() {
        assert_eq!(
            detect_version_type("a1b2c3d"),
            VersionType::Sha("a1b2c3d".to_string())
        );
    }

    #[test]
    fn full_sha1() {
        let sha = "a".repeat(40);
        assert_eq!(detect_version_type(&sha), VersionType::Sha(sha.clone()));
    }

    #[test]
    fn semver_tag() {
        assert_eq!(
            detect_version_type("v1.2.0"),
            VersionType::Tag("v1.2.0".to_string())
        );
    }

    #[test]
    fn uppercase_not_hex() {
        assert_eq!(
            detect_version_type("V1.0.0"),
            VersionType::Tag("V1.0.0".to_string())
        );
    }

    #[test]
    fn too_short_hex() {
        assert_eq!(
            detect_version_type("a1b2c3"),
            VersionType::Tag("a1b2c3".to_string())
        );
    }

    #[test]
    fn too_long_hex() {
        let long = "a".repeat(65);
        assert_eq!(detect_version_type(&long), VersionType::Tag(long.clone()));
    }
}
