use std::path::Path;

use console::style;

use graft::cache::Cache;
use graft::checksum::{checksum_bytes, checksum_directory};
use graft::config::lockfile::{LockedDep, Lockfile};
use graft::config::manifest::Manifest;
use graft::error::GraftError;
use graft::github::auth::resolve_token;
use graft::github::client::GitHubClient;
use graft::source::parse::GraftSource;

pub fn run(no_cache: bool) -> miette::Result<()> {
    let manifest_path = Path::new("graft.toml");
    let manifest = Manifest::load(manifest_path)?;

    let lockfile_path = Path::new("graft.lock");
    let mut lockfile = Lockfile::load(lockfile_path)?;

    if manifest.grafts.is_empty() {
        println!("No grafts configured.");
        return Ok(());
    }

    let token = resolve_token();
    let client = GitHubClient::new(token);
    let cache = Cache::new();

    for (name, dep) in &manifest.grafts {
        let dest_path = Path::new(&dep.dest);

        // If file already exists on disk AND is in lockfile, skip
        if dest_path.exists() && lockfile.grafts.contains_key(name) {
            println!("{} {} (already exists)", style("skip").dim(), name);
            continue;
        }

        let source = GraftSource::parse(&dep.source)?;
        let commit = client.resolve_ref(&source.owner, &source.repo, &dep.version)?;

        let checksum;
        if source.is_directory() {
            let files_filter = dep.files.as_deref();

            // Try cache first (unless --no-cache)
            let mut cached_files = Vec::new();
            let mut all_cached = false;
            if !no_cache {
                if let Some(filter) = files_filter {
                    all_cached = true;
                    for file in filter {
                        let cache_path = format!("{}{}", source.path, file);
                        if let Some(content) =
                            cache.get(&source.owner, &source.repo, &commit, &cache_path)
                        {
                            cached_files.push((file.clone(), content));
                        } else {
                            all_cached = false;
                            break;
                        }
                    }
                }
            }

            let files = if all_cached && !cached_files.is_empty() {
                cached_files
            } else {
                let fetched = client.fetch_directory(
                    &source.owner,
                    &source.repo,
                    &source.path,
                    &commit,
                    files_filter,
                )?;
                // Cache each file
                for (relative_path, content) in &fetched {
                    let cache_path = format!("{}{}", source.path, relative_path);
                    cache.put(&source.owner, &source.repo, &commit, &cache_path, content)?;
                }
                fetched
            };

            checksum = checksum_directory(
                &files
                    .iter()
                    .map(|(p, c)| (p.clone(), c.clone()))
                    .collect::<Vec<_>>(),
            );

            // Write files to dest
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

            let file_names: Vec<String> = files.iter().map(|(p, _)| p.clone()).collect();
            lockfile.grafts.insert(
                name.clone(),
                LockedDep {
                    source: dep.source.clone(),
                    version: dep.version.clone(),
                    commit,
                    checksum,
                    files: Some(file_names),
                },
            );
        } else {
            // Single file
            let content = if !no_cache {
                cache
                    .get(&source.owner, &source.repo, &commit, &source.path)
                    .unwrap_or_else(|| {
                        // Will be fetched below
                        Vec::new()
                    })
            } else {
                Vec::new()
            };

            let content = if content.is_empty() {
                let fetched =
                    client.fetch_file(&source.owner, &source.repo, &source.path, &commit)?;
                cache.put(&source.owner, &source.repo, &commit, &source.path, &fetched)?;
                fetched
            } else {
                content
            };

            checksum = checksum_bytes(&content);

            // Write file to dest
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

            lockfile.grafts.insert(
                name.clone(),
                LockedDep {
                    source: dep.source.clone(),
                    version: dep.version.clone(),
                    commit,
                    checksum,
                    files: None,
                },
            );
        }

        println!(
            "{} {} -> {}",
            style("synced").green().bold(),
            name,
            dep.dest
        );
    }

    lockfile.save(lockfile_path)?;
    Ok(())
}
