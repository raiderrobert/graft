use std::path::Path;

use console::style;

use graft::config::lockfile::Lockfile;
use graft::config::manifest::Manifest;
use graft::state::{compute_state, GraftState};

pub fn run() -> miette::Result<()> {
    let manifest_path = Path::new("graft.toml");
    let manifest = Manifest::load(manifest_path)?;

    if manifest.grafts.is_empty() {
        println!("All grafts are in sync.");
        return Ok(());
    }

    let lockfile_path = Path::new("graft.lock");
    let lockfile = Lockfile::load(lockfile_path)?;

    let project_root = Path::new(".");

    let mut problems = Vec::new();

    for (name, dep) in &manifest.grafts {
        let locked = lockfile.grafts.get(name);
        let state = compute_state(dep, locked, project_root)?;

        if state != GraftState::Synced {
            problems.push((name.clone(), state));
        }
    }

    if problems.is_empty() {
        println!("All grafts are in sync.");
    } else {
        for (name, state) in &problems {
            let state_styled = match state {
                GraftState::Modified => style(state.to_string()).yellow(),
                GraftState::Outdated => style(state.to_string()).cyan(),
                GraftState::Conflicted => style(state.to_string()).red(),
                GraftState::Missing => style(state.to_string()).red(),
                GraftState::Synced => unreachable!(),
            };
            println!("{:<20} {}", name, state_styled);
        }
        std::process::exit(1);
    }

    Ok(())
}
