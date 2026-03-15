use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use serde::de::Error as _;

use crate::error::{GraftError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Lockfile {
    #[serde(default)]
    pub grafts: IndexMap<String, LockedDep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockedDep {
    pub source: String,
    pub version: String,
    pub commit: String,
    pub checksum: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
}

impl Lockfile {
    pub fn parse(input: &str) -> Result<Self> {
        let lockfile: Lockfile =
            toml::from_str(input).map_err(|e| GraftError::LockfileParse { source: e })?;
        Ok(lockfile)
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| GraftError::LockfileParse {
            source: toml::de::Error::custom(e.to_string()),
        })
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Lockfile {
                grafts: IndexMap::new(),
            });
        }
        let contents = std::fs::read_to_string(path).map_err(|e| GraftError::Io {
            context: format!("reading {}", path.display()),
            source: e,
        })?;
        Self::parse(&contents)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        use std::io::Write;

        let contents = self.to_toml()?;
        let dir = path.parent().unwrap_or(Path::new("."));
        let mut temp = NamedTempFile::new_in(dir).map_err(|e| GraftError::Io {
            context: format!("creating temp file in {}", dir.display()),
            source: e,
        })?;
        temp.write_all(contents.as_bytes())
            .map_err(|e| GraftError::Io {
                context: "writing lockfile temp file".to_string(),
                source: e,
            })?;
        temp.persist(path).map_err(|e| GraftError::Io {
            context: format!("persisting lockfile to {}", path.display()),
            source: e.error,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_lockfile() {
        let input = r#"
[grafts.eslint]
source = "gh:eslint/eslint/.eslintrc.json"
version = "v9.0.0"
commit = "abc123def456"
checksum = "sha256:deadbeef"
"#;
        let lockfile = Lockfile::parse(input).unwrap();
        assert_eq!(lockfile.grafts.len(), 1);
        let dep = &lockfile.grafts["eslint"];
        assert_eq!(dep.commit, "abc123def456");
        assert_eq!(dep.checksum, "sha256:deadbeef");
    }

    #[test]
    fn parse_empty_lockfile() {
        let input = "";
        let lockfile = Lockfile::parse(input).unwrap();
        assert!(lockfile.grafts.is_empty());
    }

    #[test]
    fn roundtrip_serialize_deserialize() {
        let input = r#"
[grafts.eslint]
source = "gh:eslint/eslint/.eslintrc.json"
version = "v9.0.0"
commit = "abc123def456"
checksum = "sha256:deadbeef"
files = ["a.json", "b.json"]
"#;
        let lockfile = Lockfile::parse(input).unwrap();
        let serialized = lockfile.to_toml().unwrap();
        let reparsed = Lockfile::parse(&serialized).unwrap();
        assert_eq!(lockfile, reparsed);
    }
}
