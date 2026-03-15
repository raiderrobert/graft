use std::path::Path;

use console::style;

use graft::config::lockfile::Lockfile;
use graft::config::manifest::Manifest;
use graft::error::GraftError;

pub fn run(name: &str) -> miette::Result<()> {
    let manifest_path = Path::new("graft.toml");
    let mut manifest = Manifest::load(manifest_path)?;

    let lockfile_path = Path::new("graft.lock");
    let mut lockfile = Lockfile::load(lockfile_path)?;

    // Verify the graft exists
    let dep = manifest
        .grafts
        .get(name)
        .ok_or_else(|| GraftError::GraftNotFound {
            name: name.to_string(),
        })?;

    let dest = dep.dest.clone();

    // Remove from manifest
    manifest.grafts.shift_remove(name);
    manifest.save(manifest_path)?;

    // Remove from lockfile (if present)
    lockfile.grafts.shift_remove(name);
    lockfile.save(lockfile_path)?;

    println!(
        "{} {} (local file {} left in place)",
        style("removed").red().bold(),
        name,
        dest
    );

    Ok(())
}
