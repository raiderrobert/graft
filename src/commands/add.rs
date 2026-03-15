use std::path::Path;

use console::style;
use indexmap::IndexMap;

use graft::cache::Cache;
use graft::checksum::{checksum_bytes, checksum_directory};
use graft::config::lockfile::{LockedDep, Lockfile};
use graft::config::manifest::{GraftDep, Manifest};
use graft::error::GraftError;
use graft::github::auth::resolve_token;
use graft::github::client::GitHubClient;
use graft::source::parse::GraftSource;

pub fn run(source: &str, dest: Option<&str>, adopt: bool, force: bool) -> miette::Result<()> {
    // 1. Parse source@version
    let (graft_source, version) = GraftSource::parse_with_version(source)?;

    // 2. Derive a graft name
    let name = derive_name(&graft_source.path);

    // 3. Determine dest: use provided value or default to the source path
    let dest = dest.unwrap_or(&graft_source.path);

    // 4. Validate dest path before any network calls
    validate_dest(dest)?;

    // 5. Load existing manifest or create empty one
    let manifest_path = Path::new("graft.toml");
    let mut manifest = if manifest_path.exists() {
        Manifest::load(manifest_path)?
    } else {
        Manifest {
            grafts: IndexMap::new(),
        }
    };

    // 6. Check if dest already exists on disk
    let dest_path = Path::new(dest);
    if dest_path.exists() && !adopt && !force {
        return Err(GraftError::DestExists {
            path: dest_path.to_path_buf(),
        }
        .into());
    }

    // 6. Create GitHub client
    let token = resolve_token();
    let client = GitHubClient::new(token);

    // 7. Resolve version to full commit SHA
    let commit = client.resolve_ref(&graft_source.owner, &graft_source.repo, &version)?;

    // 8. Fetch content and compute checksum
    let checksum;
    if graft_source.is_directory() {
        let files = client.fetch_directory(
            &graft_source.owner,
            &graft_source.repo,
            &graft_source.path,
            &commit,
            None,
        )?;

        checksum = checksum_directory(&files);

        // 10. Write files to dest (unless --adopt)
        if !adopt {
            for (relative_path, content) in &files {
                let file_dest = dest_path.join(relative_path);
                if let Some(parent) = file_dest.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| GraftError::Io {
                        context: format!("creating directory {}", parent.display()),
                        source: e,
                    })?;
                }
                std::fs::write(&file_dest, content).map_err(|e| GraftError::Io {
                    context: format!("writing {}", file_dest.display()),
                    source: e,
                })?;
            }
        }

        // 14. Cache each file
        let cache = Cache::new();
        for (relative_path, content) in &files {
            let cache_path = format!("{}{}", graft_source.path, relative_path);
            cache.put(
                &graft_source.owner,
                &graft_source.repo,
                &commit,
                &cache_path,
                content,
            )?;
        }
    } else {
        let content = client.fetch_file(
            &graft_source.owner,
            &graft_source.repo,
            &graft_source.path,
            &commit,
        )?;

        checksum = checksum_bytes(&content);

        // 10. Write file to dest (unless --adopt)
        if !adopt {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| GraftError::Io {
                    context: format!("creating directory {}", parent.display()),
                    source: e,
                })?;
            }
            std::fs::write(dest_path, &content).map_err(|e| GraftError::Io {
                context: format!("writing {}", dest_path.display()),
                source: e,
            })?;
        }

        // 14. Cache the file
        let cache = Cache::new();
        cache.put(
            &graft_source.owner,
            &graft_source.repo,
            &commit,
            &graft_source.path,
            &content,
        )?;
    }

    // 12. Add entry to manifest and save
    manifest.grafts.insert(
        name.clone(),
        GraftDep {
            source: graft_source.to_source_string(),
            version: version.clone(),
            dest: dest.to_string(),
            files: None,
        },
    );
    manifest.save(manifest_path)?;

    // 13. Load lockfile, add entry, save
    let lockfile_path = Path::new("graft.lock");
    let mut lockfile = Lockfile::load(lockfile_path)?;
    lockfile.grafts.insert(
        name.clone(),
        LockedDep {
            source: graft_source.to_source_string(),
            version: version.clone(),
            commit,
            checksum,
            files: None,
        },
    );
    lockfile.save(lockfile_path)?;

    // 15. Print success message
    let action = if adopt { "adopted" } else { "added" };
    println!(
        "{} {} {} -> {}",
        style(action).green().bold(),
        name,
        graft_source.to_source_string(),
        dest
    );

    Ok(())
}

fn validate_dest(dest: &str) -> miette::Result<()> {
    // Check for path traversal via `..` components
    for component in Path::new(dest).components() {
        if let std::path::Component::ParentDir = component {
            return Err(GraftError::DestEscapesRoot {
                path: dest.to_string(),
            }
            .into());
        }
    }

    // Check for .git targeting
    if dest == ".git" || dest.starts_with(".git/") || dest.starts_with(".git\\") {
        return Err(GraftError::DestTargetsGit {
            path: dest.to_string(),
        }
        .into());
    }

    Ok(())
}

fn derive_name(path: &str) -> String {
    let path = path.trim_end_matches('/');
    let filename = path.rsplit('/').next().unwrap_or(path);
    filename
        .rsplit_once('.')
        .map(|(name, _)| name)
        .unwrap_or(filename)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("workflows/lint.yml", "lint")]
    #[case("skills/", "skills")]
    #[case("Makefile", "Makefile")]
    #[case(".eslintrc.json", ".eslintrc")]
    fn derive_name_from_path(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(derive_name(input), expected);
    }

    #[rstest]
    #[case(
        "gh:owner/repo/.github/workflows/release.yml@v1.0",
        ".github/workflows/release.yml"
    )]
    #[case("gh:owner/repo/Makefile@v1.0", "Makefile")]
    #[case("gh:owner/repo/skills/@v1.0", "skills/")]
    fn default_dest_from_source(#[case] source: &str, #[case] expected_dest: &str) {
        let (graft_source, _version) = GraftSource::parse_with_version(source).unwrap();
        assert_eq!(graft_source.path, expected_dest);
    }
}
