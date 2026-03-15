use std::path::Path;

use console::style;

use graft::checksum::checksum_file_on_disk;
use graft::config::lockfile::Lockfile;
use graft::config::manifest::Manifest;
use graft::error::GraftError;

pub fn run(name: &str) -> miette::Result<()> {
    let manifest_path = Path::new("graft.toml");
    let manifest = Manifest::load(manifest_path)?;

    let lockfile_path = Path::new("graft.lock");
    let mut lockfile = Lockfile::load(lockfile_path)?;

    // Verify the graft exists in manifest
    let dep = manifest
        .grafts
        .get(name)
        .ok_or_else(|| GraftError::GraftNotFound {
            name: name.to_string(),
        })?;

    // Read the file on disk
    let dest_path = Path::new(&dep.dest);
    let content = std::fs::read_to_string(dest_path).map_err(|e| GraftError::Io {
        context: format!("reading {}", dest_path.display()),
        source: e,
    })?;

    // Check for remaining conflict markers
    if content.contains("<<<<<<<") && content.contains(">>>>>>>") {
        return Err(GraftError::HasConflicts {
            name: name.to_string(),
        }
        .into());
    }

    // Compute new checksum
    let new_checksum = checksum_file_on_disk(dest_path)?;

    // Update lockfile checksum
    let locked = lockfile
        .grafts
        .get_mut(name)
        .ok_or_else(|| GraftError::GraftNotFound {
            name: name.to_string(),
        })?;
    locked.checksum = new_checksum;

    // Save lockfile atomically
    lockfile.save(lockfile_path)?;

    println!(
        "{} {} — conflict resolved",
        style("resolved").green().bold(),
        name
    );

    Ok(())
}
