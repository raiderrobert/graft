use crate::error::{GraftError, Result};

/// A parsed GitHub source reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraftSource {
    pub owner: String,
    pub repo: String,
    pub path: String,
}

impl GraftSource {
    /// Parse a source string in the format `gh:owner/repo/path/to/file`.
    ///
    /// The path portion may contain multiple `/`-separated segments.
    /// A trailing `/` indicates a directory source.
    pub fn parse(input: &str) -> Result<Self> {
        let rest = input
            .strip_prefix("gh:")
            .ok_or_else(|| GraftError::InvalidSource {
                input: input.to_string(),
            })?;

        let parts: Vec<&str> = rest.splitn(3, '/').collect();
        if parts.len() < 3 || parts[2].is_empty() {
            return Err(GraftError::InvalidSource {
                input: input.to_string(),
            });
        }

        Ok(Self {
            owner: parts[0].to_string(),
            repo: parts[1].to_string(),
            path: parts[2].to_string(),
        })
    }

    /// Parse a source string with a version suffix: `gh:owner/repo/path@version`.
    ///
    /// Splits on the last `@` to separate the version from the source.
    pub fn parse_with_version(input: &str) -> Result<(Self, String)> {
        let at_pos = input.rfind('@').ok_or_else(|| GraftError::InvalidSource {
            input: input.to_string(),
        })?;

        let source_part = &input[..at_pos];
        let version = &input[at_pos + 1..];

        if version.is_empty() {
            return Err(GraftError::InvalidSource {
                input: input.to_string(),
            });
        }

        let source = Self::parse(source_part)?;
        Ok((source, version.to_string()))
    }

    /// Return the canonical source string: `gh:{owner}/{repo}/{path}`.
    pub fn to_source_string(&self) -> String {
        format!("gh:{}/{}/{}", self.owner, self.repo, self.path)
    }

    /// Returns true if the path ends with `/`, indicating a directory source.
    pub fn is_directory(&self) -> bool {
        self.path.ends_with('/')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn parse_single_file() {
        let src = GraftSource::parse("gh:rroskam/shared-configs/workflows/lint.yml").unwrap();
        assert_eq!(src.owner, "rroskam");
        assert_eq!(src.repo, "shared-configs");
        assert_eq!(src.path, "workflows/lint.yml");
        assert!(!src.is_directory());
    }

    #[test]
    fn parse_directory() {
        let src = GraftSource::parse("gh:rroskam/shared-configs/skills/").unwrap();
        assert_eq!(src.owner, "rroskam");
        assert_eq!(src.repo, "shared-configs");
        assert_eq!(src.path, "skills/");
        assert!(src.is_directory());
    }

    #[test]
    fn parse_root_file() {
        let src = GraftSource::parse("gh:owner/repo/Makefile").unwrap();
        assert_eq!(src.owner, "owner");
        assert_eq!(src.repo, "repo");
        assert_eq!(src.path, "Makefile");
    }

    #[test]
    fn parse_with_version_tag() {
        let (src, version) = GraftSource::parse_with_version("gh:owner/repo/file@v1.2.0").unwrap();
        assert_eq!(src.owner, "owner");
        assert_eq!(src.repo, "repo");
        assert_eq!(src.path, "file");
        assert_eq!(version, "v1.2.0");
    }

    #[test]
    fn parse_with_version_sha() {
        let (src, version) = GraftSource::parse_with_version("gh:owner/repo/file@a1b2c3d").unwrap();
        assert_eq!(src.path, "file");
        assert_eq!(version, "a1b2c3d");
        assert_eq!(src.owner, "owner");
    }

    #[rstest]
    #[case("owner/repo/file")]
    #[case("gh:owner/repo")]
    fn parse_rejects_invalid(#[case] input: &str) {
        assert!(GraftSource::parse(input).is_err());
    }

    #[test]
    fn roundtrip_to_source_string() {
        let src = GraftSource::parse("gh:rroskam/shared-configs/workflows/lint.yml").unwrap();
        assert_eq!(
            src.to_source_string(),
            "gh:rroskam/shared-configs/workflows/lint.yml"
        );
    }
}
