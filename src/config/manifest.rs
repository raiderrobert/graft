use std::collections::HashSet;
use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use serde::de::Error as _;

use crate::error::{GraftError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    #[serde(default)]
    pub grafts: IndexMap<String, GraftDep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraftDep {
    pub source: String,
    #[serde(default = "default_version")]
    pub version: String,
    pub dest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
}

fn default_version() -> String {
    "latest".to_string()
}

impl Manifest {
    pub fn parse(input: &str) -> Result<Self> {
        let manifest: Manifest =
            toml::from_str(input).map_err(|e| GraftError::ManifestParse { source: e })?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<()> {
        let mut seen_dests = HashSet::new();

        for dep in self.grafts.values() {
            let dest = &dep.dest;

            // Check for path traversal via `..` components
            for component in Path::new(dest).components() {
                if let std::path::Component::ParentDir = component {
                    return Err(GraftError::DestEscapesRoot { path: dest.clone() });
                }
            }

            // Check for .git targeting
            if dest == ".git" || dest.starts_with(".git/") || dest.starts_with(".git\\") {
                return Err(GraftError::DestTargetsGit { path: dest.clone() });
            }

            // Check for duplicate dest paths
            if !seen_dests.insert(dest.clone()) {
                return Err(GraftError::DuplicateDest { path: dest.clone() });
            }
        }

        Ok(())
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| GraftError::ManifestParse {
            source: toml::de::Error::custom(e.to_string()),
        })
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(GraftError::NoManifest);
        }
        let contents = std::fs::read_to_string(path).map_err(|e| GraftError::Io {
            context: format!("reading {}", path.display()),
            source: e,
        })?;
        Self::parse(&contents)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let contents = self.to_toml()?;
        std::fs::write(path, contents).map_err(|e| GraftError::Io {
            context: format!("writing {}", path.display()),
            source: e,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_dep() {
        let input = r#"
[grafts.eslint]
source = "gh:eslint/eslint/.eslintrc.json"
version = "v9.0.0"
dest = ".eslintrc.json"
"#;
        let manifest = Manifest::parse(input).unwrap();
        assert_eq!(manifest.grafts.len(), 1);
        let dep = &manifest.grafts["eslint"];
        assert_eq!(dep.source, "gh:eslint/eslint/.eslintrc.json");
        assert_eq!(dep.version, "v9.0.0");
        assert_eq!(dep.dest, ".eslintrc.json");
        assert!(dep.files.is_none());
    }

    #[test]
    fn parse_bundle_dep_with_files() {
        let input = r#"
[grafts.prettier]
source = "gh:prettier/prettier"
version = "v3.2.0"
dest = "."
files = [".prettierrc", ".prettierignore"]
"#;
        let manifest = Manifest::parse(input).unwrap();
        let dep = &manifest.grafts["prettier"];
        assert_eq!(dep.files.as_ref().unwrap().len(), 2);
        assert_eq!(dep.files.as_ref().unwrap()[0], ".prettierrc");
    }

    #[test]
    fn parse_empty_manifest() {
        let input = "";
        let manifest = Manifest::parse(input).unwrap();
        assert!(manifest.grafts.is_empty());
    }

    #[test]
    fn reject_duplicate_dest() {
        let input = r#"
[grafts.a]
source = "gh:owner/repo/file"
dest = "config.json"

[grafts.b]
source = "gh:owner/repo2/file"
dest = "config.json"
"#;
        let err = Manifest::parse(input).unwrap_err();
        assert!(matches!(err, GraftError::DuplicateDest { .. }));
    }

    #[test]
    fn reject_dest_escaping_root() {
        let input = r#"
[grafts.evil]
source = "gh:owner/repo/file"
dest = "../etc/passwd"
"#;
        let err = Manifest::parse(input).unwrap_err();
        assert!(matches!(err, GraftError::DestEscapesRoot { .. }));
    }

    #[test]
    fn reject_dest_targeting_git() {
        let input = r#"
[grafts.hook]
source = "gh:owner/repo/hook"
dest = ".git/hooks/pre-commit"
"#;
        let err = Manifest::parse(input).unwrap_err();
        assert!(matches!(err, GraftError::DestTargetsGit { .. }));
    }

    #[test]
    fn roundtrip_serialize_deserialize() {
        let input = r#"
[grafts.eslint]
source = "gh:eslint/eslint/.eslintrc.json"
version = "v9.0.0"
dest = ".eslintrc.json"

[grafts.prettier]
source = "gh:prettier/prettier"
version = "v3.2.0"
dest = "."
files = [".prettierrc", ".prettierignore"]
"#;
        let manifest = Manifest::parse(input).unwrap();
        let serialized = manifest.to_toml().unwrap();
        let reparsed = Manifest::parse(&serialized).unwrap();
        assert_eq!(manifest, reparsed);
    }
}
