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
    use rstest::rstest;

    #[rstest]
    #[case("a1b2c3d")]
    #[case("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2")]
    fn detects_sha(#[case] input: &str) {
        assert!(matches!(detect_version_type(input), VersionType::Sha(_)));
    }

    #[rstest]
    #[case("v1.2.0")]
    #[case("V1.0.0")]
    #[case("a1b2c3")]
    #[case("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")]
    fn detects_tag(#[case] input: &str) {
        assert!(matches!(detect_version_type(input), VersionType::Tag(_)));
    }
}
