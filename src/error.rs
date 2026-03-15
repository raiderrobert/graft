#![allow(unused_assignments)]

use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum GraftError {
    #[error("No graft.toml found in current directory")]
    #[diagnostic(help("Run `graft init` to create one"))]
    NoManifest,

    #[error("Failed to parse graft.toml")]
    #[diagnostic(help("Check the TOML syntax in your graft.toml file"))]
    ManifestParse {
        #[source]
        source: toml::de::Error,
    },

    #[error("Failed to parse graft.lock")]
    #[diagnostic(help("Delete graft.lock and run `graft sync` to regenerate it"))]
    LockfileParse {
        #[source]
        source: toml::de::Error,
    },

    #[error("Invalid source format: {input}")]
    #[diagnostic(help("Use the format gh:owner/repo/path"))]
    InvalidSource { input: String },

    #[error("Duplicate destination path: {path}")]
    #[diagnostic(help("Each graft must have a unique dest path"))]
    DuplicateDest { path: String },

    #[error("Destination path escapes project root: {path}")]
    #[diagnostic(help("Destination paths must be within the project directory"))]
    DestEscapesRoot { path: String },

    #[error("Destination path targets .git directory: {path}")]
    #[diagnostic(help("Cannot graft files into the .git directory"))]
    DestTargetsGit { path: String },

    #[error("File already exists: {path}")]
    #[diagnostic(help("Use --adopt to track the existing file, or --force to overwrite"))]
    DestExists { path: PathBuf },

    #[error("Graft not found: {name}")]
    GraftNotFound { name: String },

    #[error("Could not authenticate to {repo}")]
    #[diagnostic(help("Set GH_TOKEN, run `gh auth login`, or verify the repo is public"))]
    AuthFailed { repo: String },

    #[error("Path `{path}` not found in `{repo}` at version `{version}`")]
    SourcePathNotFound {
        path: String,
        repo: String,
        version: String,
    },

    #[error("Tag `{tag}` not found in `{repo}`")]
    #[diagnostic(help("Run `git ls-remote --tags` to see available tags"))]
    TagNotFound { tag: String, repo: String },

    #[error("Network error fetching {url}")]
    #[diagnostic(help("No files were modified"))]
    Network {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("GitHub API error ({status}): {message}")]
    GitHubApi { status: u16, message: String },

    #[error("git is not installed")]
    #[diagnostic(help("Install git from https://git-scm.com"))]
    GitNotFound,

    #[error("git merge-file failed: {reason}")]
    MergeFailed { reason: String },

    #[error("IO error: {context}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Graft `{name}` has conflicts — resolve them first")]
    #[diagnostic(help(
        "Edit the file to resolve conflict markers, then run `graft resolve {name}`"
    ))]
    HasConflicts { name: String },
}

pub type Result<T> = std::result::Result<T, GraftError>;
