use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "graft", about = "Package manager for paths", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create an empty graft.toml
    Init,

    /// Add a dependency
    Add {
        /// Source and version: gh:owner/repo/path@version
        source: String,

        /// Local destination path (defaults to the source path)
        dest: Option<String>,

        /// Track an existing local file without overwriting
        #[arg(long)]
        adopt: bool,

        /// Overwrite existing destination file
        #[arg(long)]
        force: bool,
    },

    /// Fetch and write all dependencies
    Sync {
        /// Bypass the cache
        #[arg(long)]
        no_cache: bool,
    },

    /// Show all grafts with status
    List,

    /// Verify all grafts are clean (for CI)
    Check,

    /// Show grafts with newer upstream versions
    Outdated,

    /// Upgrade grafts to newer versions
    Upgrade {
        /// Specific graft to upgrade (omit for all)
        name: Option<String>,

        /// Show what would change without modifying files
        #[arg(long)]
        dry_run: bool,
    },

    /// Mark a conflicted graft as resolved
    Resolve {
        /// Graft name to resolve
        name: String,
    },

    /// Remove a graft (keeps local file)
    Remove {
        /// Graft name to remove
        name: String,
    },
}
