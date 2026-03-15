use std::path::Path;

use console::style;

use graft::config::lockfile::Lockfile;
use graft::config::manifest::Manifest;
use graft::state::{compute_state, GraftState};

pub fn run() -> miette::Result<()> {
    let manifest_path = Path::new("graft.toml");
    let manifest = Manifest::load(manifest_path)?;

    if manifest.grafts.is_empty() {
        println!("No grafts configured.");
        return Ok(());
    }

    let lockfile_path = Path::new("graft.lock");
    let lockfile = Lockfile::load(lockfile_path)?;

    let project_root = Path::new(".");

    for (name, dep) in &manifest.grafts {
        let locked = lockfile.grafts.get(name);
        let state = compute_state(dep, locked, project_root)?;

        let state_styled = match state {
            GraftState::Synced => style(state.to_string()).green(),
            GraftState::Modified => style(state.to_string()).yellow(),
            GraftState::Outdated => style(state.to_string()).cyan(),
            GraftState::Conflicted => style(state.to_string()).red(),
            GraftState::Missing => style(state.to_string()).red(),
        };

        println!(
            "{:<20} {:<16} {:<24} {}",
            name, dep.version, dep.dest, state_styled
        );
    }

    Ok(())
}
