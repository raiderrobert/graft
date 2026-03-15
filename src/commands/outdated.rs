use std::path::Path;

use console::style;

use graft::checksum::checksum_bytes;
use graft::config::lockfile::Lockfile;
use graft::config::manifest::Manifest;
use graft::github::auth::resolve_token;
use graft::github::client::GitHubClient;
use graft::source::parse::GraftSource;
use graft::source::tags::{find_newer_tags, sort_tags};
use graft::source::version::{detect_version_type, VersionType};

pub fn run() -> miette::Result<()> {
    let manifest_path = Path::new("graft.toml");
    let manifest = Manifest::load(manifest_path)?;

    if manifest.grafts.is_empty() {
        println!("All grafts are up to date.");
        return Ok(());
    }

    let lockfile_path = Path::new("graft.lock");
    let lockfile = Lockfile::load(lockfile_path)?;

    let token = resolve_token();
    let client = GitHubClient::new(token);

    let mut any_outdated = false;

    for (name, dep) in &manifest.grafts {
        // Skip SHA-pinned deps
        if let VersionType::Sha(_) = detect_version_type(&dep.version) {
            continue;
        }

        let locked = match lockfile.grafts.get(name) {
            Some(l) => l,
            None => continue, // Not yet synced
        };

        let source = GraftSource::parse(&dep.source)?;

        // Get available tags from remote
        let remote_tags = client.ls_remote_tags(&source.owner, &source.repo)?;
        let sorted = sort_tags(&remote_tags);

        // Find tags newer than current version
        let newer = find_newer_tags(&dep.version, &sorted);
        if newer.is_empty() {
            continue;
        }

        // Smart detection: check if the latest tag actually has different content
        let latest = &newer[newer.len() - 1];
        let latest_tag = &latest.0;

        // Resolve the latest tag to a commit and fetch file content
        let latest_commit = match client.resolve_ref(&source.owner, &source.repo, latest_tag) {
            Ok(c) => c,
            Err(_) => {
                // If we can't resolve, still report as outdated
                if !any_outdated {
                    any_outdated = true;
                }
                println!(
                    "{:<20} {} -> {}",
                    name,
                    style(&dep.version).dim(),
                    style(latest_tag).green().bold()
                );
                continue;
            }
        };

        // Fetch the file content at the latest tag
        let latest_checksum = if source.is_directory() {
            let files_filter = dep.files.as_deref();
            match client.fetch_directory(
                &source.owner,
                &source.repo,
                &source.path,
                &latest_commit,
                files_filter,
            ) {
                Ok(files) => graft::checksum::checksum_directory(&files),
                Err(_) => {
                    // Can't fetch, report as outdated anyway
                    String::new()
                }
            }
        } else {
            match client.fetch_file(&source.owner, &source.repo, &source.path, &latest_commit) {
                Ok(content) => checksum_bytes(&content),
                Err(_) => {
                    // Can't fetch, report as outdated anyway
                    String::new()
                }
            }
        };

        // If content is the same, skip — tag bumped but file unchanged
        if !latest_checksum.is_empty() && latest_checksum == locked.checksum {
            continue;
        }

        any_outdated = true;
        println!(
            "{:<20} {} -> {}",
            name,
            style(&dep.version).dim(),
            style(latest_tag).green().bold()
        );
    }

    if !any_outdated {
        println!("All grafts are up to date.");
    }

    Ok(())
}
