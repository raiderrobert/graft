use std::path::Path;

use console::style;

use graft::cache::Cache;
use graft::checksum::checksum_bytes;
use graft::config::lockfile::{LockedDep, Lockfile};
use graft::config::manifest::Manifest;
use graft::error::GraftError;
use graft::github::auth::resolve_token;
use graft::github::client::GitHubClient;
use graft::merge::{three_way_merge, MergeResult};
use graft::source::parse::GraftSource;
use graft::source::tags::{find_newer_tags, sort_tags};
use graft::source::version::{detect_version_type, VersionType};
use graft::state::{compute_state, GraftState};

pub fn run(name: Option<&str>, dry_run: bool) -> miette::Result<()> {
    let manifest_path = Path::new("graft.toml");
    let mut manifest = Manifest::load(manifest_path)?;

    let lockfile_path = Path::new("graft.lock");
    let mut lockfile = Lockfile::load(lockfile_path)?;

    if manifest.grafts.is_empty() {
        println!("No grafts to upgrade.");
        return Ok(());
    }

    // If a specific name is given, verify it exists
    if let Some(name) = name {
        if !manifest.grafts.contains_key(name) {
            return Err(GraftError::GraftNotFound {
                name: name.to_string(),
            }
            .into());
        }
    }

    let token = resolve_token();
    let client = GitHubClient::new(token);
    let cache = Cache::new();

    // Collect names to upgrade
    let names: Vec<String> = if let Some(name) = name {
        vec![name.to_string()]
    } else {
        manifest.grafts.keys().cloned().collect()
    };

    let mut any_upgraded = false;

    for graft_name in &names {
        let dep = manifest.grafts.get(graft_name).unwrap().clone();

        // Skip SHA-pinned deps
        if let VersionType::Sha(_) = detect_version_type(&dep.version) {
            continue;
        }

        let locked = match lockfile.grafts.get(graft_name) {
            Some(l) => l.clone(),
            None => continue, // Not yet synced
        };

        // Skip deps in conflicted state
        let state = compute_state(&dep, Some(&locked), Path::new("."))?;
        if state == GraftState::Conflicted {
            return Err(GraftError::HasConflicts {
                name: graft_name.clone(),
            }
            .into());
        }

        let source = GraftSource::parse(&dep.source)?;

        // Discover newer tags
        let remote_tags = client.ls_remote_tags(&source.owner, &source.repo)?;
        let sorted = sort_tags(&remote_tags);
        let newer = find_newer_tags(&dep.version, &sorted);

        if newer.is_empty() {
            continue;
        }

        // Find the latest newer tag
        let latest = &newer[newer.len() - 1];
        let latest_tag = latest.0.clone();

        // Resolve new tag to a commit SHA
        let new_commit = client.resolve_ref(&source.owner, &source.repo, &latest_tag)?;

        // Fetch file at new commit
        let new_content =
            client.fetch_file(&source.owner, &source.repo, &source.path, &new_commit)?;
        let new_checksum = checksum_bytes(&new_content);

        // Smart content comparison: if content unchanged despite new tag, skip
        if new_checksum == locked.checksum {
            continue;
        }

        // Get the base content: fetch file at the commit currently in lockfile
        let base_content = if let Some(cached) =
            cache.get(&source.owner, &source.repo, &locked.commit, &source.path)
        {
            cached
        } else {
            client.fetch_file(&source.owner, &source.repo, &source.path, &locked.commit)?
        };

        // Read current local file (ours)
        let dest_path = Path::new(&dep.dest);
        let ours_content = std::fs::read(dest_path).map_err(|e| GraftError::Io {
            context: format!("reading {}", dest_path.display()),
            source: e,
        })?;

        if base_content == ours_content {
            // Never customized — overwrite with new content
            if dry_run {
                println!(
                    "{} {} {} -> {}",
                    style("would upgrade").cyan(),
                    graft_name,
                    style(&dep.version).dim(),
                    style(&latest_tag).green().bold()
                );
            } else {
                std::fs::write(dest_path, &new_content).map_err(|e| GraftError::Io {
                    context: format!("writing {}", dest_path.display()),
                    source: e,
                })?;

                // Update manifest version
                manifest.grafts.get_mut(graft_name).unwrap().version = latest_tag.clone();

                // Update lockfile
                lockfile.grafts.insert(
                    graft_name.clone(),
                    LockedDep {
                        source: dep.source.clone(),
                        version: latest_tag.clone(),
                        commit: new_commit.clone(),
                        checksum: new_checksum.clone(),
                        files: locked.files.clone(),
                    },
                );

                // Cache new content
                let _ = cache.put(
                    &source.owner,
                    &source.repo,
                    &new_commit,
                    &source.path,
                    &new_content,
                );

                println!(
                    "{} {} {} -> {}",
                    style("upgraded").green().bold(),
                    graft_name,
                    style(&dep.version).dim(),
                    style(&latest_tag).green().bold()
                );
            }
        } else {
            // Locally modified — three-way merge
            let merge_result = three_way_merge(&base_content, &ours_content, &new_content)?;

            match merge_result {
                MergeResult::Clean(merged) => {
                    if dry_run {
                        println!(
                            "{} {} {} -> {} (with merge)",
                            style("would upgrade").cyan(),
                            graft_name,
                            style(&dep.version).dim(),
                            style(&latest_tag).green().bold()
                        );
                    } else {
                        let merged_checksum = checksum_bytes(&merged);
                        std::fs::write(dest_path, &merged).map_err(|e| GraftError::Io {
                            context: format!("writing {}", dest_path.display()),
                            source: e,
                        })?;

                        // Update manifest version
                        manifest.grafts.get_mut(graft_name).unwrap().version = latest_tag.clone();

                        // Update lockfile
                        lockfile.grafts.insert(
                            graft_name.clone(),
                            LockedDep {
                                source: dep.source.clone(),
                                version: latest_tag.clone(),
                                commit: new_commit.clone(),
                                checksum: merged_checksum,
                                files: locked.files.clone(),
                            },
                        );

                        // Cache new content
                        let _ = cache.put(
                            &source.owner,
                            &source.repo,
                            &new_commit,
                            &source.path,
                            &new_content,
                        );

                        println!(
                            "{} {} {} -> {} (merged local changes)",
                            style("upgraded").green().bold(),
                            graft_name,
                            style(&dep.version).dim(),
                            style(&latest_tag).green().bold()
                        );
                    }
                }
                MergeResult::Conflict(conflicted) => {
                    if dry_run {
                        println!(
                            "{} {} {} -> {} (would have conflicts)",
                            style("conflict").red().bold(),
                            graft_name,
                            style(&dep.version).dim(),
                            style(&latest_tag).green().bold()
                        );
                    } else {
                        // Write conflict markers to file
                        std::fs::write(dest_path, &conflicted).map_err(|e| GraftError::Io {
                            context: format!("writing {}", dest_path.display()),
                            source: e,
                        })?;

                        // Update lockfile version and commit but NOT checksum
                        // so the conflict state is detectable
                        lockfile.grafts.insert(
                            graft_name.clone(),
                            LockedDep {
                                source: dep.source.clone(),
                                version: latest_tag.clone(),
                                commit: new_commit.clone(),
                                checksum: locked.checksum.clone(),
                                files: locked.files.clone(),
                            },
                        );

                        // Update manifest version
                        manifest.grafts.get_mut(graft_name).unwrap().version = latest_tag.clone();

                        // Cache new content
                        let _ = cache.put(
                            &source.owner,
                            &source.repo,
                            &new_commit,
                            &source.path,
                            &new_content,
                        );

                        println!(
                            "{} {} — resolve conflicts in {}, then run `graft resolve {}`",
                            style("conflict").red().bold(),
                            graft_name,
                            dep.dest,
                            graft_name
                        );
                    }
                }
            }
        }

        any_upgraded = true;
    }

    if !dry_run && any_upgraded {
        manifest.save(manifest_path)?;
        lockfile.save(lockfile_path)?;
    }

    if !any_upgraded {
        println!("All grafts are up to date.");
    }

    Ok(())
}
