mod cli;
mod commands;

use clap::Parser;
use cli::{Cli, Commands};

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => commands::init::run(),
        Commands::Add {
            source,
            dest,
            adopt,
            force,
        } => commands::add::run(&source, dest.as_deref(), adopt, force),
        Commands::Sync { no_cache } => commands::sync::run(no_cache),
        Commands::List => commands::list::run(),
        Commands::Check => commands::check::run(),
        Commands::Outdated => commands::outdated::run(),
        Commands::Upgrade { name, dry_run } => commands::upgrade::run(name.as_deref(), dry_run),
        Commands::Resolve { name } => commands::resolve::run(&name),
        Commands::Remove { name } => commands::remove::run(&name),
    }
}
