# Graft Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox syntax for tracking.

**Goal:** Build graft v0.1.0 — a CLI tool that manages versioned file dependencies from GitHub repos with three-way merge on upgrade.

**Architecture:** Single Rust crate with both library (`src/lib.rs`) and binary (`src/main.rs`) targets. Modules: `cli` (clap derive), `commands/` (one file per subcommand), `config/` (manifest + lockfile types), `source/` (gh: parsing, version detection), `github/` (API client, auth), `cache/` (XDG cache layer), `checksum/` (SHA-256), `merge/` (git merge-file wrapper), `error.rs` (thiserror + miette). All I/O is blocking (no async). File writes use atomic temp-then-rename via `tempfile` crate.

**Tech Stack:** Rust (2021 edition), clap 4 (derive), serde + toml, reqwest (blocking, rustls-tls), sha2, tempfile, thiserror + miette, dirs, console. Dev: rstest, cucumber (BDD with Gherkin `.feature` files). Requires `git` on PATH for `git merge-file` and `git ls-remote`.

---

## Task 1: Project Scaffolding

**Files to create:**
- `~/repos/graft/Cargo.toml`
- `~/repos/graft/src/main.rs`
- `~/repos/graft/src/lib.rs`
- `~/repos/graft/src/error.rs`
- `~/repos/graft/src/cli.rs`
- `~/repos/graft/justfile`
- `~/repos/graft/.github/workflows/ci.yml`
- `~/repos/graft/.gitignore`
- `~/repos/graft/CLAUDE.md`
- `~/repos/graft/LICENSE` (MIT)

### Steps

- [ ] **1.1** Create repo directory and initialize git:
  ```bash
  mkdir -p ~/repos/graft && cd ~/repos/graft && git init
  ```

- [ ] **1.2** Create `~/repos/graft/Cargo.toml`:
  ```toml
  [workspace]

  [package]
  name = "graft"
  version = "0.1.0"
  edition = "2021"
  license = "MIT"
  rust-version = "1.75"
  description = "A package manager for config files"

  [lib]
  name = "graft"
  path = "src/lib.rs"

  [[bin]]
  name = "graft"
  path = "src/main.rs"

  [dependencies]
  clap = { version = "4", features = ["derive"] }
  serde = { version = "1", features = ["derive"] }
  toml = "0.8"
  thiserror = "2"
  miette = { version = "7", features = ["fancy"] }
  reqwest = { version = "0.12", features = ["blocking", "rustls-tls"], default-features = false }
  sha2 = "0.10"
  tempfile = "3"
  dirs = "6"
  console = "0.15"
  semver = "1"
  flate2 = "1"
  tar = "0.4"
  hex = "0.4"
  base64 = "0.22"
  serde_json = "1"
  indexmap = { version = "2", features = ["serde"] }

  [dev-dependencies]
  rstest = "0.23"
  cucumber = "0.21"
  async-trait = "0.1"       # Required by cucumber derive macros
  tokio = { version = "1", features = ["macros", "rt-multi-thread"] }  # cucumber's async test runner

  [[test]]
  name = "bdd"
  harness = false            # cucumber provides its own test harness
  ```

- [ ] **1.3** Create `~/repos/graft/src/error.rs`:
  ```rust
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
      #[diagnostic(help(
          "Set GH_TOKEN, run `gh auth login`, or verify the repo is public"
      ))]
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
      #[diagnostic(help("Edit the file to resolve conflict markers, then run `graft resolve {name}`"))]
      HasConflicts { name: String },
  }

  pub type Result<T> = std::result::Result<T, GraftError>;
  ```

- [ ] **1.4** Create `~/repos/graft/src/cli.rs`:
  ```rust
  use clap::{Parser, Subcommand};

  #[derive(Parser)]
  #[command(
      name = "graft",
      about = "A package manager for config files",
      version
  )]
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

          /// Local destination path
          dest: String,

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
  ```

- [ ] **1.5** Create `~/repos/graft/src/lib.rs`:
  ```rust
  pub mod error;
  ```

- [ ] **1.6** Create `~/repos/graft/src/main.rs`:
  ```rust
  mod cli;
  mod commands;

  use clap::Parser;
  use cli::{Cli, Commands};

  fn main() -> miette::Result<()> {
      let cli = Cli::parse();
      match cli.command {
          Commands::Init => commands::init::run(),
          Commands::Add { source, dest, adopt, force } => {
              commands::add::run(&source, &dest, adopt, force)
          }
          Commands::Sync { no_cache } => commands::sync::run(no_cache),
          Commands::List => commands::list::run(),
          Commands::Check => commands::check::run(),
          Commands::Outdated => commands::outdated::run(),
          Commands::Upgrade { name, dry_run } => commands::upgrade::run(name.as_deref(), dry_run),
          Commands::Resolve { name } => commands::resolve::run(&name),
          Commands::Remove { name } => commands::remove::run(&name),
      }
  }
  ```

- [ ] **1.7** Create `~/repos/graft/src/commands/mod.rs` with stub modules:
  ```rust
  pub mod init;
  pub mod add;
  pub mod sync;
  pub mod list;
  pub mod check;
  pub mod outdated;
  pub mod upgrade;
  pub mod resolve;
  pub mod remove;
  ```

- [ ] **1.8** Create stub command files. Each file in `~/repos/graft/src/commands/{init,add,sync,list,check,outdated,upgrade,resolve,remove}.rs`:
  ```rust
  pub fn run(/* args */) -> miette::Result<()> {
      todo!()
  }
  ```
  Make sure each stub's signature matches what `main.rs` calls.

- [ ] **1.9** Create `~/repos/graft/justfile`:
  ```justfile
  # List available recipes
  default:
      @just --list

  # Run all checks (fmt, clippy, test)
  check:
      cargo fmt --check
      cargo clippy -- -D warnings
      cargo test

  # Run tests
  test *args:
      cargo test {{args}}

  # Build release binary
  build:
      cargo build --release

  # Run formatting
  fmt:
      cargo fmt

  # Install graft locally
  install:
      cargo install --path .
  ```

- [ ] **1.10** Create `~/repos/graft/.github/workflows/ci.yml`:
  ```yaml
  name: CI

  on:
    push:
      branches: [main]
      paths:
        - "src/**"
        - "Cargo.toml"
        - "Cargo.lock"
        - "justfile"
        - ".github/workflows/ci.yml"
    pull_request:
      branches: [main]
      paths:
        - "src/**"
        - "Cargo.toml"
        - "Cargo.lock"
        - "justfile"
        - ".github/workflows/ci.yml"

  env:
    CARGO_TERM_COLOR: always
    RUSTFLAGS: -Dwarnings

  jobs:
    check:
      name: Check, Lint & Test
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4
        - uses: dtolnay/rust-toolchain@stable
          with:
            components: rustfmt, clippy
        - uses: extractions/setup-just@v2
        - uses: Swatinem/rust-cache@v2
        - name: Run checks
          run: just check
  ```

- [ ] **1.11** Create `~/repos/graft/.gitignore`:
  ```
  # Rust
  *.profraw
  *.profdata
  /target

  # Common tooling
  .worktrees/
  .beads/
  .claude/
  CLAUDE.md

  # macOS
  .DS_Store

  # IDEs
  .vscode/
  .idea/
  ```

- [ ] **1.12** Create `~/repos/graft/CLAUDE.md`:
  ```markdown
  # Graft

  Rust CLI tool — a package manager for config files. Manages versioned file dependencies from GitHub repos with three-way merge on upgrade.

  ## Project Structure

  Single crate with both library (`src/lib.rs`) and binary (`src/main.rs`) targets.
  - `src/lib.rs` — library root (config, source resolution, GitHub client, cache, checksum, merge)
  - `src/main.rs` — CLI entry point using clap
  - `src/cli.rs` — clap derive command definitions
  - `src/commands/` — CLI subcommand handlers
  - `src/config/` — manifest (graft.toml) and lockfile (graft.lock) types
  - `src/source/` — gh: shorthand parsing, version type detection
  - `src/github/` — GitHub API client, auth chain
  - `src/cache.rs` — XDG cache layer
  - `src/checksum.rs` — SHA-256 computation
  - `src/merge.rs` — git merge-file wrapper
  - `src/error.rs` — error types (thiserror + miette)
  - `tests/` — integration tests

  ## Pre-Commit Validation

  Run these checks before every commit. Fix failures before committing.

  ```
  cargo fmt --check
  cargo clippy -- -D warnings
  cargo test
  ```

  ## Conventions

  - Use `thiserror` for error types in `src/error.rs`
  - `error.rs` has `#![allow(unused_assignments)]` — this is a known thiserror/clippy false positive, do not remove
  - Blocking reqwest only (no async)
  - Atomic file writes via `tempfile::NamedTempFile::persist()`
  - Requires `git` on PATH for `git merge-file` and `git ls-remote`
  ```

- [ ] **1.13** Verify it compiles:
  ```bash
  cd ~/repos/graft && cargo build
  ```

- [ ] **1.14** Run pre-commit checks:
  ```bash
  cd ~/repos/graft && cargo fmt --check && cargo clippy -- -D warnings && cargo test
  ```

- [ ] **1.15** Create BDD test runner at `~/repos/graft/tests/bdd.rs`:
  ```rust
  use cucumber::World;
  use std::path::PathBuf;

  #[derive(Debug, Default, World)]
  pub struct GraftWorld {
      /// Temp directory for test isolation
      pub work_dir: Option<PathBuf>,
      /// Last command's exit code
      pub exit_code: Option<i32>,
      /// Last command's stdout
      pub stdout: String,
      /// Last command's stderr
      pub stderr: String,
  }

  fn main() {
      futures::executor::block_on(
          GraftWorld::run("tests/features"),
      );
  }
  ```

- [ ] **1.16** Create BDD feature files directory and initial feature files:

  Create `~/repos/graft/tests/features/init.feature`:
  ```gherkin
  Feature: Initialize a project with graft

    Scenario: Create a new graft.toml in an empty directory
      Given an empty project directory
      When I run "graft init"
      Then the command should succeed
      And a file "graft.toml" should exist
      And "graft.toml" should contain "# Graft"

    Scenario: Init is idempotent when graft.toml already exists
      Given an empty project directory
      And a file "graft.toml" with content "# my existing config"
      When I run "graft init"
      Then the command should succeed
      And "graft.toml" should contain "# my existing config"
  ```

  Create `~/repos/graft/tests/features/add.feature`:
  ```gherkin
  Feature: Add a file dependency from a GitHub repo

    Scenario: Add a new file graft
      Given an empty project directory with "graft.toml"
      When I run "graft add gh:owner/repo/Makefile@v1.0.0 Makefile"
      Then the command should succeed
      And a file "Makefile" should exist
      And "graft.toml" should contain "source"
      And a file "graft.lock" should exist

    Scenario: Add refuses when destination already exists
      Given an empty project directory with "graft.toml"
      And a file "Makefile" with content "existing content"
      When I run "graft add gh:owner/repo/Makefile@v1.0.0 Makefile"
      Then the command should fail
      And stderr should contain "already exists"

    Scenario: Add with --adopt preserves local file
      Given an empty project directory with "graft.toml"
      And a file "Makefile" with content "my local version"
      When I run "graft add gh:owner/repo/Makefile@v1.0.0 Makefile --adopt"
      Then the command should succeed
      And "Makefile" should contain "my local version"
      And "graft.toml" should contain "source"

    Scenario: Add rejects path traversal
      Given an empty project directory with "graft.toml"
      When I run "graft add gh:owner/repo/evil@v1.0.0 ../etc/passwd"
      Then the command should fail
      And stderr should contain "escapes project root"

    Scenario: Add rejects .git directory targets
      Given an empty project directory with "graft.toml"
      When I run "graft add gh:owner/repo/hook@v1.0.0 .git/hooks/pre-commit"
      Then the command should fail
      And stderr should contain ".git"
  ```

  Create `~/repos/graft/tests/features/list.feature`:
  ```gherkin
  Feature: List grafts and their status

    Scenario: List shows synced grafts
      Given a project with a synced graft "lint" at version "v1.0.0"
      When I run "graft list"
      Then the command should succeed
      And stdout should contain "lint"
      And stdout should contain "synced"

    Scenario: List shows modified grafts
      Given a project with a synced graft "lint" at version "v1.0.0"
      And I modify the grafted file "lint"
      When I run "graft list"
      Then the command should succeed
      And stdout should contain "modified"

    Scenario: List with no grafts
      Given an empty project directory with "graft.toml"
      When I run "graft list"
      Then the command should succeed
  ```

  Create `~/repos/graft/tests/features/check.feature`:
  ```gherkin
  Feature: Check graft status for CI

    Scenario: Check succeeds when all grafts are synced
      Given a project with a synced graft "lint" at version "v1.0.0"
      When I run "graft check"
      Then the exit code should be 0

    Scenario: Check fails when a graft is modified
      Given a project with a synced graft "lint" at version "v1.0.0"
      And I modify the grafted file "lint"
      When I run "graft check"
      Then the exit code should be 1
  ```

  Create `~/repos/graft/tests/features/upgrade.feature`:
  ```gherkin
  Feature: Upgrade grafts to newer versions

    Scenario: Upgrade overwrites unmodified file
      Given a project with a synced graft "lint" at version "v1.0.0"
      And upstream has a newer version "v1.1.0" with different content
      When I run "graft upgrade lint"
      Then the command should succeed
      And the grafted file should contain the v1.1.0 content
      And "graft.lock" should contain "v1.1.0"

    Scenario: Upgrade merges locally modified file
      Given a project with a synced graft "lint" at version "v1.0.0"
      And I modify the grafted file "lint"
      And upstream has a newer version "v1.1.0" with non-conflicting changes
      When I run "graft upgrade lint"
      Then the command should succeed
      And the grafted file should contain both local and upstream changes

    Scenario: Upgrade with conflicts leaves conflict markers
      Given a project with a synced graft "lint" at version "v1.0.0"
      And I modify the grafted file "lint" on the same lines as upstream
      And upstream has a newer version "v1.1.0" with conflicting changes
      When I run "graft upgrade lint"
      Then the grafted file should contain conflict markers
      And "graft.lock" version should still be "v1.0.0"

    Scenario: Dry run shows changes without modifying files
      Given a project with a synced graft "lint" at version "v1.0.0"
      And upstream has a newer version "v1.1.0" with different content
      When I run "graft upgrade lint --dry-run"
      Then the command should succeed
      And the grafted file should contain the v1.0.0 content
      And "graft.lock" should contain "v1.0.0"
  ```

  Create `~/repos/graft/tests/features/resolve.feature`:
  ```gherkin
  Feature: Resolve conflicts after upgrade

    Scenario: Resolve updates lockfile after manual conflict resolution
      Given a project with a conflicted graft "lint"
      And I manually resolve the conflicts in the grafted file
      When I run "graft resolve lint"
      Then the command should succeed
      And "graft.lock" should be updated with the new checksum

    Scenario: Resolve rejects file that still has conflict markers
      Given a project with a conflicted graft "lint"
      When I run "graft resolve lint"
      Then the command should fail
      And stderr should contain "conflict"
  ```

  Create `~/repos/graft/tests/features/remove.feature`:
  ```gherkin
  Feature: Remove a graft

    Scenario: Remove deletes from manifest but keeps local file
      Given a project with a synced graft "lint" at version "v1.0.0"
      When I run "graft remove lint"
      Then the command should succeed
      And "graft.toml" should not contain "lint"
      And "graft.lock" should not contain "lint"
      And the local file for "lint" should still exist

    Scenario: Remove nonexistent graft fails
      Given an empty project directory with "graft.toml"
      When I run "graft remove nonexistent"
      Then the command should fail
      And stderr should contain "not found"
  ```

  Create `~/repos/graft/tests/features/outdated.feature`:
  ```gherkin
  Feature: Check for outdated grafts

    Scenario: Shows outdated graft when upstream has newer version
      Given a project with a synced graft "lint" at version "v1.0.0"
      And upstream has a newer version "v1.1.0" with different content
      When I run "graft outdated"
      Then the command should succeed
      And stdout should contain "v1.0.0"
      And stdout should contain "v1.1.0"

    Scenario: Skips SHA-pinned grafts
      Given a project with a synced graft "config" pinned to SHA "a1b2c3d"
      When I run "graft outdated"
      Then the command should succeed
      And stdout should not contain "config"

    Scenario: Skips grafts where file content is unchanged at new tag
      Given a project with a synced graft "lint" at version "v1.0.0"
      And upstream has a newer tag "v1.1.0" but the file content is identical
      When I run "graft outdated"
      Then the command should succeed
      And stdout should not contain "lint"
  ```

- [ ] **1.17** Verify the BDD runner compiles (tests will be pending since steps aren't implemented yet):
  ```bash
  cd ~/repos/graft && cargo test --test bdd
  ```

- [ ] **1.18** Commit:
  ```
  chore: scaffold project structure with BDD feature files
  ```

---

## Task 2: Config Types (Manifest + Lockfile)

**Files to create/modify:**
- `~/repos/graft/src/config/mod.rs`
- `~/repos/graft/src/config/manifest.rs`
- `~/repos/graft/src/config/lockfile.rs`
- `~/repos/graft/src/lib.rs` (add `pub mod config`)

### Steps

- [ ] **2.1** Create `~/repos/graft/src/config/mod.rs`:
  ```rust
  pub mod manifest;
  pub mod lockfile;
  ```

- [ ] **2.2** Write failing tests in `~/repos/graft/src/config/manifest.rs`. Test: parse a valid graft.toml string into `Manifest` struct, validate duplicate dests are rejected, validate dest path escaping is rejected, validate dest targeting `.git/` is rejected.
  ```rust
  use indexmap::IndexMap;
  use serde::{Deserialize, Serialize};
  use std::path::Path;
  use crate::error::{GraftError, Result};

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct Manifest {
      #[serde(default)]
      pub deps: IndexMap<String, GraftDep>,
  }

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct GraftDep {
      pub source: String,
      pub version: String,
      pub dest: String,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub files: Option<Vec<String>>,
  }

  impl Manifest {
      pub fn parse(input: &str) -> Result<Self> {
          let manifest: Manifest = toml::from_str(input).map_err(|e| GraftError::ManifestParse { source: e })?;
          manifest.validate()?;
          Ok(manifest)
      }

      pub fn validate(&self) -> Result<()> {
          let mut seen_dests = std::collections::HashSet::new();
          for (name, dep) in &self.deps {
              // Check for duplicate dest
              if !seen_dests.insert(&dep.dest) {
                  return Err(GraftError::DuplicateDest { path: dep.dest.clone() });
              }
              // Check dest doesn't escape project root
              let path = Path::new(&dep.dest);
              for component in path.components() {
                  if matches!(component, std::path::Component::ParentDir) {
                      return Err(GraftError::DestEscapesRoot { path: dep.dest.clone() });
                  }
              }
              // Check dest doesn't target .git/
              if dep.dest == ".git" || dep.dest.starts_with(".git/") || dep.dest.starts_with(".git\\") {
                  return Err(GraftError::DestTargetsGit { path: dep.dest.clone() });
              }
          }
          Ok(())
      }

      pub fn to_toml(&self) -> Result<String> {
          toml::to_string_pretty(self).map_err(|e| GraftError::Io {
              context: "serializing manifest".into(),
              source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
          })
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_parse_valid_manifest() {
          let input = r#"
  [deps.ci-lint]
  source = "gh:rroskam/shared-configs/workflows/lint.yml"
  version = "v1.2.0"
  dest = ".github/workflows/lint.yml"

  [deps.claude-skills]
  source = "gh:rroskam/shared-configs/skills/"
  version = "v2.0.0"
  dest = ".claude/skills/"
  files = ["brainstorming.md", "debugging.md"]
  "#;
          let manifest = Manifest::parse(input).unwrap();
          assert_eq!(manifest.deps.len(), 2);
          assert_eq!(manifest.deps["ci-lint"].version, "v1.2.0");
          assert_eq!(manifest.deps["claude-skills"].files.as_ref().unwrap().len(), 2);
      }

      #[test]
      fn test_empty_manifest() {
          let input = "";
          let manifest = Manifest::parse(input).unwrap();
          assert_eq!(manifest.deps.len(), 0);
      }

      #[test]
      fn test_duplicate_dest_rejected() {
          let input = r#"
  [deps.a]
  source = "gh:owner/repo/file1"
  version = "v1.0.0"
  dest = "same/path"

  [deps.b]
  source = "gh:owner/repo/file2"
  version = "v1.0.0"
  dest = "same/path"
  "#;
          let err = Manifest::parse(input).unwrap_err();
          assert!(matches!(err, GraftError::DuplicateDest { .. }));
      }

      #[test]
      fn test_dest_escaping_root_rejected() {
          let input = r#"
  [deps.evil]
  source = "gh:owner/repo/file"
  version = "v1.0.0"
  dest = "../etc/passwd"
  "#;
          let err = Manifest::parse(input).unwrap_err();
          assert!(matches!(err, GraftError::DestEscapesRoot { .. }));
      }

      #[test]
      fn test_dest_targeting_git_rejected() {
          let input = r#"
  [deps.evil]
  source = "gh:owner/repo/file"
  version = "v1.0.0"
  dest = ".git/hooks/pre-commit"
  "#;
          let err = Manifest::parse(input).unwrap_err();
          assert!(matches!(err, GraftError::DestTargetsGit { .. }));
      }

      #[test]
      fn test_roundtrip_serialize() {
          let input = r#"
  [deps.ci-lint]
  source = "gh:rroskam/shared-configs/workflows/lint.yml"
  version = "v1.2.0"
  dest = ".github/workflows/lint.yml"
  "#;
          let manifest = Manifest::parse(input).unwrap();
          let output = manifest.to_toml().unwrap();
          let reparsed = Manifest::parse(&output).unwrap();
          assert_eq!(manifest.deps.len(), reparsed.deps.len());
      }
  }
  ```

- [ ] **2.3** Verify tests fail (module not wired up yet), then add `pub mod config` to `src/lib.rs`. Verify tests compile and pass:
  ```bash
  cd ~/repos/graft && cargo test config::manifest
  ```

- [ ] **2.4** Write `~/repos/graft/src/config/lockfile.rs` with tests:
  ```rust
  use indexmap::IndexMap;
  use serde::{Deserialize, Serialize};
  use crate::error::{GraftError, Result};

  #[derive(Debug, Clone, Serialize, Deserialize, Default)]
  pub struct Lockfile {
      #[serde(default)]
      pub deps: IndexMap<String, LockedDep>,
  }

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct LockedDep {
      pub source: String,
      pub version: String,
      pub commit: String,
      pub checksum: String,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub files: Option<Vec<String>>,
  }

  impl Lockfile {
      pub fn parse(input: &str) -> Result<Self> {
          toml::from_str(input).map_err(|e| GraftError::LockfileParse { source: e })
      }

      pub fn to_toml(&self) -> Result<String> {
          toml::to_string_pretty(self).map_err(|e| GraftError::Io {
              context: "serializing lockfile".into(),
              source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
          })
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_parse_valid_lockfile() {
          let input = r#"
  [deps.ci-lint]
  source = "gh:rroskam/shared-configs/workflows/lint.yml"
  version = "v1.2.0"
  commit = "f4e5d6c7a8b9000000000000000000000000beef"
  checksum = "sha256:abc123"

  [deps.claude-skills]
  source = "gh:rroskam/shared-configs/skills/"
  version = "v2.0.0"
  commit = "1a2b3c4d5e6f000000000000000000000000cafe"
  checksum = "sha256:aaa111"
  files = ["brainstorming.md", "debugging.md"]
  "#;
          let lock = Lockfile::parse(input).unwrap();
          assert_eq!(lock.deps.len(), 2);
          assert_eq!(lock.deps["ci-lint"].commit, "f4e5d6c7a8b9000000000000000000000000beef");
          assert!(lock.deps["claude-skills"].files.is_some());
      }

      #[test]
      fn test_empty_lockfile() {
          let input = "";
          let lock = Lockfile::parse(input).unwrap();
          assert_eq!(lock.deps.len(), 0);
      }

      #[test]
      fn test_roundtrip_serialize() {
          let input = r#"
  [deps.ci-lint]
  source = "gh:rroskam/shared-configs/workflows/lint.yml"
  version = "v1.2.0"
  commit = "f4e5d6c7a8b9000000000000000000000000beef"
  checksum = "sha256:abc123"
  "#;
          let lock = Lockfile::parse(input).unwrap();
          let output = lock.to_toml().unwrap();
          let reparsed = Lockfile::parse(&output).unwrap();
          assert_eq!(lock.deps.len(), reparsed.deps.len());
      }
  }
  ```

- [ ] **2.5** Run tests:
  ```bash
  cd ~/repos/graft && cargo test config::
  ```

- [ ] **2.6** Add helper functions to `manifest.rs` and `lockfile.rs` for reading from / writing to disk paths. Use atomic writes for lockfile:
  ```rust
  // In manifest.rs
  impl Manifest {
      pub fn load(path: &Path) -> Result<Self> { /* read_to_string + parse */ }
      pub fn save(&self, path: &Path) -> Result<()> { /* to_toml + write */ }
  }

  // In lockfile.rs
  impl Lockfile {
      pub fn load(path: &Path) -> Result<Self> { /* read_to_string + parse, or default if missing */ }
      pub fn save(&self, path: &Path) -> Result<()> { /* atomic write via tempfile */ }
  }
  ```

- [ ] **2.7** Run full checks:
  ```bash
  cd ~/repos/graft && cargo fmt --check && cargo clippy -- -D warnings && cargo test
  ```

- [ ] **2.8** Commit:
  ```
  feat: add manifest and lockfile config types with validation
  ```

---

## Task 3: Source Resolution

**Files to create/modify:**
- `~/repos/graft/src/source/mod.rs`
- `~/repos/graft/src/source/parse.rs`
- `~/repos/graft/src/source/version.rs`
- `~/repos/graft/src/lib.rs` (add `pub mod source`)

### Steps

- [ ] **3.1** Create `~/repos/graft/src/source/mod.rs`:
  ```rust
  pub mod parse;
  pub mod version;
  ```

- [ ] **3.2** Write failing tests first in `~/repos/graft/src/source/parse.rs`, then implement. Must parse `gh:owner/repo/path` into a `GraftSource` struct with fields: `owner`, `repo`, `path`. Also parse the `source@version` format used by `graft add` (splits on `@`). Tests:
  ```rust
  #[derive(Debug, Clone, PartialEq)]
  pub struct GraftSource {
      pub owner: String,
      pub repo: String,
      pub path: String,
  }

  impl GraftSource {
      /// Parse "gh:owner/repo/path/to/file" into components.
      pub fn parse(input: &str) -> Result<Self> { /* ... */ }

      /// Parse "gh:owner/repo/path@version" into (source, version).
      pub fn parse_with_version(input: &str) -> Result<(Self, String)> { /* ... */ }

      /// Return the "gh:owner/repo/path" canonical form.
      pub fn to_source_string(&self) -> String { /* ... */ }

      /// Is this a directory source? (path ends with /)
      pub fn is_directory(&self) -> bool { /* ... */ }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_parse_single_file() {
          let s = GraftSource::parse("gh:rroskam/shared-configs/workflows/lint.yml").unwrap();
          assert_eq!(s.owner, "rroskam");
          assert_eq!(s.repo, "shared-configs");
          assert_eq!(s.path, "workflows/lint.yml");
      }

      #[test]
      fn test_parse_directory() {
          let s = GraftSource::parse("gh:rroskam/shared-configs/skills/").unwrap();
          assert_eq!(s.path, "skills/");
          assert!(s.is_directory());
      }

      #[test]
      fn test_parse_root_file() {
          let s = GraftSource::parse("gh:owner/repo/Makefile").unwrap();
          assert_eq!(s.path, "Makefile");
      }

      #[test]
      fn test_parse_with_version() {
          let (s, v) = GraftSource::parse_with_version("gh:owner/repo/file@v1.2.0").unwrap();
          assert_eq!(s.path, "file");
          assert_eq!(v, "v1.2.0");
      }

      #[test]
      fn test_parse_with_sha_version() {
          let (s, v) = GraftSource::parse_with_version("gh:owner/repo/file@a1b2c3d").unwrap();
          assert_eq!(v, "a1b2c3d");
      }

      #[test]
      fn test_invalid_no_prefix() {
          assert!(GraftSource::parse("owner/repo/file").is_err());
      }

      #[test]
      fn test_invalid_too_few_parts() {
          assert!(GraftSource::parse("gh:owner/repo").is_err());
      }

      #[test]
      fn test_to_source_string() {
          let s = GraftSource::parse("gh:rroskam/configs/file.yml").unwrap();
          assert_eq!(s.to_source_string(), "gh:rroskam/configs/file.yml");
      }
  }
  ```

- [ ] **3.3** Wire up module in `src/lib.rs`, verify tests pass:
  ```bash
  cd ~/repos/graft && cargo test source::parse
  ```

- [ ] **3.4** Write failing tests first in `~/repos/graft/src/source/version.rs`, then implement. Must detect version type (tag vs SHA) and validate:
  ```rust
  #[derive(Debug, Clone, PartialEq)]
  pub enum VersionType {
      Tag(String),
      Sha(String),
  }

  /// Detect whether a version string is a SHA or a tag.
  /// SHA: matches ^[0-9a-f]{7,64}$
  /// Everything else: tag.
  pub fn detect_version_type(version: &str) -> VersionType { /* ... */ }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_sha_short() {
          assert_eq!(detect_version_type("a1b2c3d"), VersionType::Sha("a1b2c3d".into()));
      }

      #[test]
      fn test_sha_full_40() {
          let sha = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
          assert_eq!(detect_version_type(sha), VersionType::Sha(sha.into()));
      }

      #[test]
      fn test_tag_semver() {
          assert_eq!(detect_version_type("v1.2.0"), VersionType::Tag("v1.2.0".into()));
      }

      #[test]
      fn test_tag_with_uppercase() {
          // Has uppercase, so not a valid hex SHA
          assert_eq!(detect_version_type("V1.0.0"), VersionType::Tag("V1.0.0".into()));
      }

      #[test]
      fn test_sha_6_chars_too_short() {
          // 6 chars — below minimum of 7
          assert_eq!(detect_version_type("a1b2c3"), VersionType::Tag("a1b2c3".into()));
      }

      #[test]
      fn test_tag_that_looks_hex_but_too_long() {
          let too_long = "a".repeat(65);
          assert_eq!(detect_version_type(&too_long), VersionType::Tag(too_long.clone()));
      }
  }
  ```

- [ ] **3.5** Run tests:
  ```bash
  cd ~/repos/graft && cargo test source::version
  ```

- [ ] **3.6** Run full checks:
  ```bash
  cd ~/repos/graft && cargo fmt --check && cargo clippy -- -D warnings && cargo test
  ```

- [ ] **3.7** Commit:
  ```
  feat: add source parsing and version type detection
  ```

---

## Task 4: GitHub API Client

**Files to create/modify:**
- `~/repos/graft/src/github/mod.rs`
- `~/repos/graft/src/github/auth.rs`
- `~/repos/graft/src/github/client.rs`
- `~/repos/graft/src/lib.rs` (add `pub mod github`)

### Steps

- [ ] **4.1** Create `~/repos/graft/src/github/mod.rs`:
  ```rust
  pub mod auth;
  pub mod client;
  ```

- [ ] **4.2** Write `~/repos/graft/src/github/auth.rs` with tests. Implements the auth chain: (1) `GH_TOKEN` env var, (2) `GITHUB_TOKEN` env var, (3) `gh auth token` output, (4) None.
  ```rust
  use std::process::Command;

  /// Resolve a GitHub token from the environment or gh CLI.
  /// Returns None if no token is found (unauthenticated / public repos only).
  pub fn resolve_token() -> Option<String> {
      // 1. GH_TOKEN env var
      if let Ok(token) = std::env::var("GH_TOKEN") {
          if !token.is_empty() {
              return Some(token);
          }
      }

      // 2. GITHUB_TOKEN env var
      if let Ok(token) = std::env::var("GITHUB_TOKEN") {
          if !token.is_empty() {
              return Some(token);
          }
      }

      // 3. gh auth token
      if let Ok(output) = Command::new("gh").args(["auth", "token"]).output() {
          if output.status.success() {
              let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
              if !token.is_empty() {
                  return Some(token);
              }
          }
      }

      None
  }

  #[cfg(test)]
  mod tests {
      // Auth tests are limited since they depend on environment.
      // We test the function doesn't panic and returns Option.
      use super::*;

      #[test]
      fn test_resolve_token_returns_option() {
          // This just verifies the function runs without panicking.
          // Actual token presence depends on the environment.
          let _result = resolve_token();
      }
  }
  ```

- [ ] **4.3** Write `~/repos/graft/src/github/client.rs`. This is the main GitHub API module. Implement:
  - `GitHubClient` struct wrapping `reqwest::blocking::Client` + optional token
  - `resolve_ref(&self, owner, repo, git_ref) -> Result<String>` — resolve a tag/SHA to a full commit SHA. Use `GET /repos/{owner}/{repo}/git/ref/tags/{tag}` or `GET /repos/{owner}/{repo}/commits/{sha}`. For annotated tags, dereference to the commit.
  - `fetch_file(&self, owner, repo, path, git_ref) -> Result<Vec<u8>>` — fetch a single file via Contents API, decode base64.
  - `fetch_directory(&self, owner, repo, path, git_ref, files_filter) -> Result<Vec<(String, Vec<u8>)>>` — download tarball, extract matching paths.
  - `ls_remote_tags(&self, owner, repo) -> Result<Vec<(String, String)>>` — shell out to `git ls-remote --tags`, parse output, handle `^{}` dereferencing.

  Tests for this module use `#[ignore]` since they require network/auth. Include at least:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_client_creation() {
          let client = GitHubClient::new(None);
          // Just verifying construction doesn't panic
          assert!(true);
      }

      #[test]
      fn test_client_creation_with_token() {
          let client = GitHubClient::new(Some("fake-token".into()));
          assert!(true);
      }

      // Network-dependent tests
      #[test]
      #[ignore]
      fn test_fetch_file_from_public_repo() {
          let client = GitHubClient::new(None);
          let content = client.fetch_file("rroskam", "shared-configs", "README.md", "main").unwrap();
          assert!(!content.is_empty());
      }
  }
  ```

- [ ] **4.4** Wire up in `src/lib.rs`, verify it compiles and unit tests pass:
  ```bash
  cd ~/repos/graft && cargo test github::
  ```

- [ ] **4.5** Run full checks:
  ```bash
  cd ~/repos/graft && cargo fmt --check && cargo clippy -- -D warnings && cargo test
  ```

- [ ] **4.6** Commit:
  ```
  feat: add GitHub API client with auth chain
  ```

---

## Task 5: Cache Layer

**Files to create/modify:**
- `~/repos/graft/src/cache.rs`
- `~/repos/graft/src/lib.rs` (add `pub mod cache`)

### Steps

- [ ] **5.1** Write failing tests first in `~/repos/graft/src/cache.rs`, then implement. The cache stores fetched file content keyed by `source+commit`. Cache dir: `~/.graft/cache/`, overridable via `GRAFT_CACHE_DIR`. Key format: `{owner}/{repo}/{commit}/{path}`. Atomic writes via tempfile.

  ```rust
  use std::path::{Path, PathBuf};
  use crate::error::{GraftError, Result};

  pub struct Cache {
      root: PathBuf,
  }

  impl Cache {
      pub fn new() -> Self {
          let root = if let Ok(dir) = std::env::var("GRAFT_CACHE_DIR") {
              PathBuf::from(dir)
          } else {
              dirs::home_dir()
                  .unwrap_or_else(|| PathBuf::from("."))
                  .join(".graft")
                  .join("cache")
          };
          Self { root }
      }

      #[cfg(test)]
      pub fn with_root(root: PathBuf) -> Self {
          Self { root }
      }

      /// Cache key path for a given source + commit + file path.
      pub fn key_path(&self, owner: &str, repo: &str, commit: &str, path: &str) -> PathBuf {
          self.root.join(owner).join(repo).join(commit).join(path)
      }

      /// Get cached content if it exists.
      pub fn get(&self, owner: &str, repo: &str, commit: &str, path: &str) -> Option<Vec<u8>> {
          let key = self.key_path(owner, repo, commit, path);
          std::fs::read(&key).ok()
      }

      /// Store content in the cache with atomic write.
      pub fn put(&self, owner: &str, repo: &str, commit: &str, path: &str, content: &[u8]) -> Result<()> {
          let key = self.key_path(owner, repo, commit, path);
          if let Some(parent) = key.parent() {
              std::fs::create_dir_all(parent).map_err(|e| GraftError::Io {
                  context: format!("creating cache directory {}", parent.display()),
                  source: e,
              })?;
          }
          // Atomic write
          let mut tmp = tempfile::NamedTempFile::new_in(key.parent().unwrap())
              .map_err(|e| GraftError::Io {
                  context: "creating temp file for cache".into(),
                  source: e,
              })?;
          std::io::Write::write_all(&mut tmp, content).map_err(|e| GraftError::Io {
              context: "writing cache content".into(),
              source: e,
          })?;
          tmp.persist(&key).map_err(|e| GraftError::Io {
              context: format!("persisting cache file {}", key.display()),
              source: e.error,
          })?;
          Ok(())
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_cache_miss() {
          let dir = tempfile::tempdir().unwrap();
          let cache = Cache::with_root(dir.path().to_path_buf());
          assert!(cache.get("owner", "repo", "abc123", "file.yml").is_none());
      }

      #[test]
      fn test_cache_put_and_get() {
          let dir = tempfile::tempdir().unwrap();
          let cache = Cache::with_root(dir.path().to_path_buf());
          cache.put("owner", "repo", "abc123", "file.yml", b"content").unwrap();
          let result = cache.get("owner", "repo", "abc123", "file.yml");
          assert_eq!(result, Some(b"content".to_vec()));
      }

      #[test]
      fn test_cache_nested_path() {
          let dir = tempfile::tempdir().unwrap();
          let cache = Cache::with_root(dir.path().to_path_buf());
          cache.put("owner", "repo", "abc123", "workflows/lint.yml", b"yaml").unwrap();
          assert_eq!(cache.get("owner", "repo", "abc123", "workflows/lint.yml"), Some(b"yaml".to_vec()));
      }

      #[test]
      fn test_cache_different_commits() {
          let dir = tempfile::tempdir().unwrap();
          let cache = Cache::with_root(dir.path().to_path_buf());
          cache.put("owner", "repo", "commit1", "file", b"v1").unwrap();
          cache.put("owner", "repo", "commit2", "file", b"v2").unwrap();
          assert_eq!(cache.get("owner", "repo", "commit1", "file"), Some(b"v1".to_vec()));
          assert_eq!(cache.get("owner", "repo", "commit2", "file"), Some(b"v2".to_vec()));
      }
  }
  ```

- [ ] **5.2** Wire up in `src/lib.rs`, run tests:
  ```bash
  cd ~/repos/graft && cargo test cache::
  ```

- [ ] **5.3** Run full checks and commit:
  ```
  feat: add file cache with atomic writes
  ```

---

## Task 6: Checksum Computation

**Files to create/modify:**
- `~/repos/graft/src/checksum.rs`
- `~/repos/graft/src/lib.rs` (add `pub mod checksum`)

### Steps

- [ ] **6.1** Write failing tests first in `~/repos/graft/src/checksum.rs`, then implement:
  - `checksum_file(content: &[u8]) -> String` — returns `"sha256:{hex}"`.
  - `checksum_directory(files: &[(String, Vec<u8>)]) -> String` — sort filenames by full relative path lexicographically, compute each file's SHA-256, concatenate all hex hashes, hash the concatenation. Returns `"sha256:{hex}"`.
  - `checksum_file_on_disk(path: &Path) -> Result<String>` — read file, compute checksum.
  - `checksum_directory_on_disk(dir: &Path, files: &[String]) -> Result<String>` — read each file, compute directory checksum.

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_checksum_file() {
          let cs = checksum_bytes(b"hello world");
          assert!(cs.starts_with("sha256:"));
          assert_eq!(cs.len(), 7 + 64); // "sha256:" + 64 hex chars
      }

      #[test]
      fn test_checksum_deterministic() {
          let a = checksum_bytes(b"test content");
          let b = checksum_bytes(b"test content");
          assert_eq!(a, b);
      }

      #[test]
      fn test_checksum_different_content() {
          let a = checksum_bytes(b"content a");
          let b = checksum_bytes(b"content b");
          assert_ne!(a, b);
      }

      #[test]
      fn test_checksum_directory() {
          let files = vec![
              ("b.txt".to_string(), b"beta".to_vec()),
              ("a.txt".to_string(), b"alpha".to_vec()),
          ];
          let cs = checksum_directory(&files);
          assert!(cs.starts_with("sha256:"));

          // Order shouldn't matter — it sorts internally
          let files_reversed = vec![
              ("a.txt".to_string(), b"alpha".to_vec()),
              ("b.txt".to_string(), b"beta".to_vec()),
          ];
          assert_eq!(cs, checksum_directory(&files_reversed));
      }

      #[test]
      fn test_checksum_file_on_disk() {
          let dir = tempfile::tempdir().unwrap();
          let path = dir.path().join("test.txt");
          std::fs::write(&path, b"file content").unwrap();
          let cs = checksum_file_on_disk(&path).unwrap();
          assert_eq!(cs, checksum_bytes(b"file content"));
      }
  }
  ```

- [ ] **6.2** Wire up, run tests:
  ```bash
  cd ~/repos/graft && cargo test checksum::
  ```

- [ ] **6.3** Run full checks and commit:
  ```
  feat: add SHA-256 checksum computation for files and directories
  ```

---

## Task 7: `graft init` Command

**Files to modify:**
- `~/repos/graft/src/commands/init.rs`

### Steps

- [ ] **7.1** Implement `graft init` — create an empty `graft.toml` with a helpful comment. If `graft.toml` already exists, print a message and return Ok.
  ```rust
  use std::path::Path;
  use console::style;

  pub fn run() -> miette::Result<()> {
      let path = Path::new("graft.toml");
      if path.exists() {
          println!("{} graft.toml already exists", style("skip").yellow().bold());
          return Ok(());
      }

      let content = r#"# Graft — package manager for config files
  # Docs: https://github.com/rroskam/graft
  #
  # [deps.example]
  # source = "gh:owner/repo/path/to/file"
  # version = "v1.0.0"
  # dest = "local/path/to/file"
  "#;

      std::fs::write(path, content).map_err(|e| graft::error::GraftError::Io {
          context: "writing graft.toml".into(),
          source: e,
      })?;

      println!("{} Created graft.toml", style("done").green().bold());
      Ok(())
  }
  ```

- [ ] **7.2** Write an integration test in `~/repos/graft/tests/init.rs`:
  ```rust
  use std::fs;
  use tempfile::TempDir;
  use std::process::Command;

  fn graft_bin() -> Command {
      Command::new(env!("CARGO_BIN_EXE_graft"))
  }

  #[test]
  fn test_init_creates_manifest() {
      let dir = TempDir::new().unwrap();
      let output = graft_bin()
          .arg("init")
          .current_dir(dir.path())
          .output()
          .unwrap();
      assert!(output.status.success());
      assert!(dir.path().join("graft.toml").exists());
  }

  #[test]
  fn test_init_idempotent() {
      let dir = TempDir::new().unwrap();
      fs::write(dir.path().join("graft.toml"), "# existing").unwrap();
      let output = graft_bin()
          .arg("init")
          .current_dir(dir.path())
          .output()
          .unwrap();
      assert!(output.status.success());
      // Should not overwrite
      let content = fs::read_to_string(dir.path().join("graft.toml")).unwrap();
      assert_eq!(content, "# existing");
  }
  ```

- [ ] **7.3** Run tests:
  ```bash
  cd ~/repos/graft && cargo test --test init
  ```

- [ ] **7.4** Run full checks and commit:
  ```
  feat: implement graft init command
  ```

---

## Task 8: `graft add` Command

**Files to modify:**
- `~/repos/graft/src/commands/add.rs`

### Steps

- [ ] **8.1** Implement the core `add` logic. This is the most complex command so far. Steps:
  1. Parse `source@version` via `GraftSource::parse_with_version`
  2. Derive a graft name from the source path (filename without extension, or last dir component)
  3. Load existing manifest (or create empty)
  4. Validate dest is safe (no `..`, no `.git`)
  5. Check if dest already exists on disk — if so, require `--adopt` or `--force`
  6. Create `GitHubClient`, resolve the ref to a full commit SHA
  7. Fetch the file content (single file or directory)
  8. Compute checksum of fetched content
  9. If not `--adopt`: write content to dest (create parent dirs)
  10. Add entry to manifest, save
  11. Add entry to lockfile, save (atomic)
  12. Print success message

  Key helper — derive graft name from path:
  ```rust
  fn derive_name(path: &str) -> String {
      let path = path.trim_end_matches('/');
      let filename = path.rsplit('/').next().unwrap_or(path);
      // Strip extension
      filename.rsplit_once('.').map(|(name, _)| name).unwrap_or(filename).to_string()
  }
  ```

- [ ] **8.2** Write unit tests for `derive_name`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_derive_name_from_file() {
          assert_eq!(derive_name("workflows/lint.yml"), "lint");
      }

      #[test]
      fn test_derive_name_from_dir() {
          assert_eq!(derive_name("skills/"), "skills");
      }

      #[test]
      fn test_derive_name_root_file() {
          assert_eq!(derive_name("Makefile"), "Makefile");
      }

      #[test]
      fn test_derive_name_dotfile() {
          assert_eq!(derive_name(".eslintrc.json"), ".eslintrc");
      }
  }
  ```

- [ ] **8.3** Write an integration test in `~/repos/graft/tests/add.rs` that tests the `--adopt` path (no network needed — mock by pre-creating the manifest and lockfile entries manually, or use `#[ignore]` for network tests):
  ```rust
  #[test]
  #[ignore] // Requires network + auth
  fn test_add_fetches_and_writes() {
      // Run `graft add gh:owner/repo/file@tag dest` in a temp dir
      // Verify graft.toml and graft.lock are updated
      // Verify dest file exists
  }
  ```

- [ ] **8.4** Run tests:
  ```bash
  cd ~/repos/graft && cargo test commands::add
  ```

- [ ] **8.5** Run full checks and commit:
  ```
  feat: implement graft add command with --adopt and --force flags
  ```

---

## Task 9: `graft sync` Command

**Files to modify:**
- `~/repos/graft/src/commands/sync.rs`

### Steps

- [ ] **9.1** Implement `graft sync`. For each dep in `graft.toml`:
  1. If already in lockfile and file exists on disk — skip
  2. If not in lockfile or file missing — resolve ref, fetch content, write file, update lockfile
  3. Respect `--no-cache` flag
  4. Report what was synced

- [ ] **9.2** Write integration test in `~/repos/graft/tests/sync.rs`:
  ```rust
  #[test]
  fn test_sync_writes_missing_files() {
      // Create a temp dir with a graft.toml + graft.lock (pre-populated)
      // Delete the dest file
      // Run graft sync
      // Verify file is restored
  }
  ```

- [ ] **9.3** Run full checks and commit:
  ```
  feat: implement graft sync command
  ```

---

## Task 10: `graft list` Command

**Files to modify:**
- `~/repos/graft/src/commands/list.rs`

### Steps

- [ ] **10.1** Implement `graft list`. For each dep in manifest:
  1. Determine state: synced (checksum matches lockfile), modified (checksum differs), conflicted (conflict markers present in file — detect `<<<<<<<` markers), or missing (dest doesn't exist).
  2. Print table: name, version, dest, state (colored).

  Extract state computation into a reusable function in `src/lib.rs` or a new `src/state.rs`:
  ```rust
  #[derive(Debug, Clone, PartialEq)]
  pub enum GraftState {
      Synced,
      Modified,
      Outdated,
      Conflicted,
      Missing,
  }

  pub fn compute_state(dep: &GraftDep, locked: Option<&LockedDep>, project_root: &Path) -> Result<GraftState> {
      let dest = project_root.join(&dep.dest);
      if !dest.exists() {
          return Ok(GraftState::Missing);
      }
      let Some(locked) = locked else {
          return Ok(GraftState::Missing);
      };
      // Check for conflict markers
      if let Ok(content) = std::fs::read_to_string(&dest) {
          if content.contains("<<<<<<<") && content.contains(">>>>>>>") {
              return Ok(GraftState::Conflicted);
          }
      }
      // Compare checksum
      let current_checksum = if dep.dest.ends_with('/') || dest.is_dir() {
          let files = locked.files.as_deref().unwrap_or(&[]);
          checksum_directory_on_disk(&dest, files)?
      } else {
          checksum_file_on_disk(&dest)?
      };
      if current_checksum == locked.checksum {
          Ok(GraftState::Synced)
      } else {
          Ok(GraftState::Modified)
      }
  }
  ```

- [ ] **10.2** Write unit tests for `compute_state`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_state_missing_file() {
          let dir = tempfile::tempdir().unwrap();
          let dep = GraftDep {
              source: "gh:o/r/f".into(),
              version: "v1".into(),
              dest: "nonexistent.txt".into(),
              files: None,
          };
          let state = compute_state(&dep, None, dir.path()).unwrap();
          assert_eq!(state, GraftState::Missing);
      }

      #[test]
      fn test_state_synced() {
          let dir = tempfile::tempdir().unwrap();
          let content = b"hello world";
          std::fs::write(dir.path().join("file.txt"), content).unwrap();
          let cs = checksum_bytes(content);
          let dep = GraftDep {
              source: "gh:o/r/f".into(),
              version: "v1".into(),
              dest: "file.txt".into(),
              files: None,
          };
          let locked = LockedDep {
              source: "gh:o/r/f".into(),
              version: "v1".into(),
              commit: "abc".into(),
              checksum: cs,
              files: None,
          };
          let state = compute_state(&dep, Some(&locked), dir.path()).unwrap();
          assert_eq!(state, GraftState::Synced);
      }

      #[test]
      fn test_state_modified() {
          let dir = tempfile::tempdir().unwrap();
          std::fs::write(dir.path().join("file.txt"), b"modified").unwrap();
          let dep = GraftDep {
              source: "gh:o/r/f".into(),
              version: "v1".into(),
              dest: "file.txt".into(),
              files: None,
          };
          let locked = LockedDep {
              source: "gh:o/r/f".into(),
              version: "v1".into(),
              commit: "abc".into(),
              checksum: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
              files: None,
          };
          let state = compute_state(&dep, Some(&locked), dir.path()).unwrap();
          assert_eq!(state, GraftState::Modified);
      }

      #[test]
      fn test_state_conflicted() {
          let dir = tempfile::tempdir().unwrap();
          let content = "before\n<<<<<<< ours\nour line\n=======\ntheir line\n>>>>>>> theirs\nafter";
          std::fs::write(dir.path().join("file.txt"), content).unwrap();
          let dep = GraftDep {
              source: "gh:o/r/f".into(),
              version: "v1".into(),
              dest: "file.txt".into(),
              files: None,
          };
          let locked = LockedDep {
              source: "gh:o/r/f".into(),
              version: "v1".into(),
              commit: "abc".into(),
              checksum: "sha256:different".into(),
              files: None,
          };
          let state = compute_state(&dep, Some(&locked), dir.path()).unwrap();
          assert_eq!(state, GraftState::Conflicted);
      }
  }
  ```

- [ ] **10.3** Implement the `list` command display using `console` crate for colored output.

- [ ] **10.4** Run full checks and commit:
  ```
  feat: implement graft list command with state computation
  ```

---

## Task 11: `graft check` Command

**Files to modify:**
- `~/repos/graft/src/commands/check.rs`

### Steps

- [ ] **11.1** Implement `graft check`. Reuses `compute_state`. Exit code 0 if all synced, exit code 1 otherwise. Print problematic grafts.

  ```rust
  pub fn run() -> miette::Result<()> {
      let manifest = Manifest::load(Path::new("graft.toml"))?;
      let lockfile = Lockfile::load(Path::new("graft.lock"))?;
      let project_root = std::env::current_dir().map_err(|e| GraftError::Io {
          context: "getting current directory".into(),
          source: e,
      })?;

      let mut all_clean = true;
      for (name, dep) in &manifest.deps {
          let locked = lockfile.deps.get(name);
          let state = compute_state(dep, locked, &project_root)?;
          if state != GraftState::Synced {
              all_clean = false;
              // Print the problematic graft
          }
      }

      if !all_clean {
          std::process::exit(1);
      }
      Ok(())
  }
  ```

- [ ] **11.2** Write integration test in `~/repos/graft/tests/check.rs` verifying exit codes.

- [ ] **11.3** Run full checks and commit:
  ```
  feat: implement graft check command for CI
  ```

---

## Task 12: `graft outdated` Command

**Files to create/modify:**
- `~/repos/graft/src/commands/outdated.rs`
- `~/repos/graft/src/source/tags.rs` (tag sorting logic)
- `~/repos/graft/src/source/mod.rs` (add `pub mod tags`)

### Steps

- [ ] **12.1** Write `~/repos/graft/src/source/tags.rs` with failing tests first. Implement:
  - `parse_ls_remote_tags(output: &str) -> Vec<(String, String)>` — parse `git ls-remote --tags` output, handling `^{}` dereferencing (prefer `^{}` SHA when present).
  - `sort_tags(tags: &[(String, String)]) -> Vec<(String, String)>` — sort by semver when possible (strip leading `v`), fall back to lexicographic.
  - `find_newer_tags(current: &str, tags: &[(String, String)]) -> Vec<(String, String)>` — return tags newer than `current`.

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_parse_ls_remote_simple() {
          let output = "abc123\trefs/tags/v1.0.0\ndef456\trefs/tags/v1.1.0\n";
          let tags = parse_ls_remote_tags(output);
          assert_eq!(tags.len(), 2);
          assert_eq!(tags[0], ("v1.0.0".into(), "abc123".into()));
      }

      #[test]
      fn test_parse_ls_remote_annotated_tags() {
          let output = "abc123\trefs/tags/v1.0.0\nfff999\trefs/tags/v1.0.0^{}\n";
          let tags = parse_ls_remote_tags(output);
          assert_eq!(tags.len(), 1);
          // Should use the ^{} (dereferenced) SHA
          assert_eq!(tags[0], ("v1.0.0".into(), "fff999".into()));
      }

      #[test]
      fn test_sort_tags_semver() {
          let tags = vec![
              ("v1.1.0".into(), "a".into()),
              ("v1.0.0".into(), "b".into()),
              ("v1.2.0".into(), "c".into()),
          ];
          let sorted = sort_tags(&tags);
          assert_eq!(sorted[0].0, "v1.0.0");
          assert_eq!(sorted[1].0, "v1.1.0");
          assert_eq!(sorted[2].0, "v1.2.0");
      }

      #[test]
      fn test_sort_tags_mixed() {
          // Mix of semver and non-semver — non-semver sorts lexicographically after semver
          let tags = vec![
              ("release-2".into(), "a".into()),
              ("v1.0.0".into(), "b".into()),
              ("release-1".into(), "c".into()),
          ];
          let sorted = sort_tags(&tags);
          // semver first, then non-semver lexicographic
          assert_eq!(sorted[0].0, "v1.0.0");
          assert_eq!(sorted[1].0, "release-1");
          assert_eq!(sorted[2].0, "release-2");
      }

      #[test]
      fn test_find_newer_tags() {
          let tags = vec![
              ("v1.0.0".into(), "a".into()),
              ("v1.1.0".into(), "b".into()),
              ("v1.2.0".into(), "c".into()),
              ("v2.0.0".into(), "d".into()),
          ];
          let newer = find_newer_tags("v1.1.0", &tags);
          assert_eq!(newer.len(), 2);
          assert_eq!(newer[0].0, "v1.2.0");
          assert_eq!(newer[1].0, "v2.0.0");
      }
  }
  ```

- [ ] **12.2** Run tests:
  ```bash
  cd ~/repos/graft && cargo test source::tags
  ```

- [ ] **12.3** Implement `graft outdated` command. For each dep with a tag version:
  1. Run `git ls-remote --tags` (via `GitHubClient::ls_remote_tags`)
  2. Find newer tags
  3. For each newer tag (starting from latest): fetch file at that tag, compare checksum with lockfile. If content differs, report as outdated with the latest differing tag.
  4. Skip SHA-pinned deps.

- [ ] **12.4** Run full checks and commit:
  ```
  feat: implement graft outdated command with smart content comparison
  ```

---

## Task 13: Three-Way Merge Engine

**Files to create/modify:**
- `~/repos/graft/src/merge.rs`
- `~/repos/graft/src/lib.rs` (add `pub mod merge`)

### Steps

- [ ] **13.1** Write failing tests first in `~/repos/graft/src/merge.rs`, then implement. Wraps `git merge-file`:
  ```rust
  use std::path::Path;
  use std::process::Command;
  use crate::error::{GraftError, Result};

  #[derive(Debug, PartialEq)]
  pub enum MergeResult {
      /// Clean merge — merged content returned.
      Clean(Vec<u8>),
      /// Conflicts — content with conflict markers returned.
      Conflict(Vec<u8>),
  }

  /// Three-way merge using git merge-file.
  ///
  /// - base: the original upstream content (from locked commit)
  /// - ours: the current local file content
  /// - theirs: the new upstream content
  ///
  /// Returns MergeResult::Clean if no conflicts, MergeResult::Conflict if conflicts.
  pub fn three_way_merge(base: &[u8], ours: &[u8], theirs: &[u8]) -> Result<MergeResult> {
      // Write all three to temp files
      let dir = tempfile::tempdir().map_err(|e| GraftError::Io {
          context: "creating temp dir for merge".into(),
          source: e,
      })?;

      let base_path = dir.path().join("base");
      let ours_path = dir.path().join("ours");
      let theirs_path = dir.path().join("theirs");

      std::fs::write(&base_path, base).map_err(|e| GraftError::Io {
          context: "writing base for merge".into(),
          source: e,
      })?;
      std::fs::write(&ours_path, ours).map_err(|e| GraftError::Io {
          context: "writing ours for merge".into(),
          source: e,
      })?;
      std::fs::write(&theirs_path, theirs).map_err(|e| GraftError::Io {
          context: "writing theirs for merge".into(),
          source: e,
      })?;

      // git merge-file modifies "ours" in place
      // Exit code 0 = clean merge, 1 = conflicts, <0 = error
      let output = Command::new("git")
          .args(["merge-file", "-p"])
          .arg(&ours_path)
          .arg(&base_path)
          .arg(&theirs_path)
          .output()
          .map_err(|e| {
              if e.kind() == std::io::ErrorKind::NotFound {
                  GraftError::GitNotFound
              } else {
                  GraftError::Io {
                      context: "running git merge-file".into(),
                      source: e,
                  }
              }
          })?;

      let merged = output.stdout;

      if output.status.success() {
          Ok(MergeResult::Clean(merged))
      } else if output.status.code() == Some(1) {
          Ok(MergeResult::Conflict(merged))
      } else {
          Err(GraftError::MergeFailed {
              reason: String::from_utf8_lossy(&output.stderr).to_string(),
          })
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_clean_merge_no_changes() {
          let base = b"line1\nline2\nline3\n";
          let ours = b"line1\nline2\nline3\n";
          let theirs = b"line1\nline2\nline3\n";
          let result = three_way_merge(base, ours, theirs).unwrap();
          assert!(matches!(result, MergeResult::Clean(_)));
      }

      #[test]
      fn test_clean_merge_non_overlapping() {
          let base = b"line1\nline2\nline3\n";
          let ours = b"line1 modified\nline2\nline3\n";
          let theirs = b"line1\nline2\nline3 modified\n";
          let result = three_way_merge(base, ours, theirs).unwrap();
          match result {
              MergeResult::Clean(content) => {
                  let s = String::from_utf8(content).unwrap();
                  assert!(s.contains("line1 modified"));
                  assert!(s.contains("line3 modified"));
              }
              MergeResult::Conflict(_) => panic!("Expected clean merge"),
          }
      }

      #[test]
      fn test_conflict_overlapping() {
          let base = b"line1\nline2\nline3\n";
          let ours = b"line1\nours change\nline3\n";
          let theirs = b"line1\ntheirs change\nline3\n";
          let result = three_way_merge(base, ours, theirs).unwrap();
          match result {
              MergeResult::Conflict(content) => {
                  let s = String::from_utf8(content).unwrap();
                  assert!(s.contains("<<<<<<<"));
                  assert!(s.contains(">>>>>>>"));
              }
              MergeResult::Clean(_) => panic!("Expected conflict"),
          }
      }

      #[test]
      fn test_theirs_only_change() {
          let base = b"original\n";
          let ours = b"original\n";
          let theirs = b"updated\n";
          let result = three_way_merge(base, ours, theirs).unwrap();
          match result {
              MergeResult::Clean(content) => {
                  assert_eq!(content, b"updated\n");
              }
              MergeResult::Conflict(_) => panic!("Expected clean merge"),
          }
      }

      #[test]
      fn test_ours_only_change() {
          let base = b"original\n";
          let ours = b"our edit\n";
          let theirs = b"original\n";
          let result = three_way_merge(base, ours, theirs).unwrap();
          match result {
              MergeResult::Clean(content) => {
                  assert_eq!(content, b"our edit\n");
              }
              MergeResult::Conflict(_) => panic!("Expected clean merge"),
          }
      }
  }
  ```

- [ ] **13.2** Wire up in `src/lib.rs`, run tests:
  ```bash
  cd ~/repos/graft && cargo test merge::
  ```

- [ ] **13.3** Run full checks and commit:
  ```
  feat: add three-way merge engine wrapping git merge-file
  ```

---

## Task 14: `graft upgrade` Command

**Files to modify:**
- `~/repos/graft/src/commands/upgrade.rs`

### Steps

- [ ] **14.1** Implement `graft upgrade [name] [--dry-run]`. For each graft to upgrade:
  1. Skip SHA-pinned deps
  2. Run `ls_remote_tags` to find newer tags
  3. Smart content comparison: fetch file at latest tag, compare with locked checksum. Skip if content unchanged.
  4. Fetch base (content at locked commit) from cache or GitHub
  5. Read current local file (ours)
  6. Fetch new upstream content (theirs)
  7. If base == ours: overwrite with theirs (no local modifications)
  8. If base != ours: three-way merge
     - Clean merge: write merged content, update lockfile
     - Conflict: write conflict markers, do NOT update lockfile version/checksum
  9. If `--dry-run`: print what would happen, don't write anything
  10. Update manifest version to new tag
  11. Update lockfile (new commit, new checksum)

- [ ] **14.2** Write unit tests for the upgrade decision logic (not the full command, just the "should we upgrade?" and "merge strategy" decisions):
  ```rust
  #[cfg(test)]
  mod tests {
      #[test]
      fn test_skip_sha_pinned() {
          // SHA version should be skipped
      }

      #[test]
      fn test_skip_content_unchanged() {
          // New tag but same file content — no upgrade needed
      }
  }
  ```

- [ ] **14.3** Run full checks and commit:
  ```
  feat: implement graft upgrade command with three-way merge
  ```

---

## Task 15: `graft resolve` Command

**Files to modify:**
- `~/repos/graft/src/commands/resolve.rs`

### Steps

- [ ] **15.1** Implement `graft resolve <name>`:
  1. Load manifest + lockfile
  2. Verify the named graft exists
  3. Read the file on disk
  4. Verify no conflict markers remain (error if `<<<<<<<` still present)
  5. Compute new checksum of resolved file
  6. Update lockfile with the new version (from manifest, since upgrade updated it) and new checksum
  7. Save lockfile atomically

- [ ] **15.2** Write a unit test:
  ```rust
  #[test]
  fn test_resolve_rejects_unresolved_conflicts() {
      // File still has conflict markers — should error
  }

  #[test]
  fn test_resolve_updates_lockfile() {
      // File is clean — lockfile should be updated
  }
  ```

- [ ] **15.3** Run full checks and commit:
  ```
  feat: implement graft resolve command
  ```

---

## Task 16: `graft remove` Command

**Files to modify:**
- `~/repos/graft/src/commands/remove.rs`

### Steps

- [ ] **16.1** Implement `graft remove <name>`:
  1. Load manifest + lockfile
  2. Verify the named graft exists in the manifest
  3. Remove from manifest, save
  4. Remove from lockfile, save atomically
  5. Do NOT delete the local file — print a message saying the file was left in place

- [ ] **16.2** Write integration test in `~/repos/graft/tests/remove.rs`:
  ```rust
  #[test]
  fn test_remove_keeps_local_file() {
      let dir = TempDir::new().unwrap();
      // Set up graft.toml, graft.lock, and dest file
      // Run graft remove
      // Verify graft.toml and graft.lock no longer contain the entry
      // Verify dest file still exists
  }

  #[test]
  fn test_remove_nonexistent_graft() {
      // Should error with GraftNotFound
  }
  ```

- [ ] **16.3** Run full checks and commit:
  ```
  feat: implement graft remove command
  ```

---

## Task 17: `graft add --adopt` Flag (Detailed)

This was outlined in Task 8 but deserves its own verification step since `--adopt` is a nuanced feature.

### Steps

- [ ] **17.1** Write a focused integration test for `--adopt` behavior:
  ```rust
  #[test]
  fn test_add_adopt_preserves_local_file() {
      let dir = TempDir::new().unwrap();
      // Pre-create graft.toml (empty)
      // Pre-create the dest file with custom content
      // Run: graft add gh:owner/repo/file@tag dest --adopt
      // Verify:
      //   - graft.toml has the entry
      //   - graft.lock has the entry with upstream checksum (NOT local file checksum)
      //   - dest file content is UNCHANGED (still the local content)
      //   - graft list shows "modified" state
  }
  ```

- [ ] **17.2** Verify `--adopt` flows correctly end-to-end. Ensure the lockfile records the upstream checksum (not the local file's checksum), so the first `graft upgrade` will properly three-way merge.

- [ ] **17.3** Run full checks and commit (if changes were needed):
  ```
  test: add integration tests for graft add --adopt
  ```

---

## Task 18: Final Polish

### Steps

- [ ] **18.1** Run the full test suite:
  ```bash
  cd ~/repos/graft && cargo fmt --check && cargo clippy -- -D warnings && cargo test
  ```

- [ ] **18.2** Run `cargo build --release` and verify the binary works:
  ```bash
  cd ~/repos/graft && cargo build --release
  ./target/release/graft --version
  ./target/release/graft --help
  ./target/release/graft init
  ```

- [ ] **18.3** Create the GitHub repo and push:
  ```bash
  cd ~/repos/graft && gh repo create rroskam/graft --private --source=. --push
  ```

- [ ] **18.4** Verify CI passes on GitHub.

---

## BDD Step Implementation Strategy

Feature files are written upfront in Task 1. Step definitions are implemented alongside each command:

- **Task 7 (graft init):** Implement steps for `init.feature` — `Given an empty project directory`, `When I run "graft ..."`, `Then the command should succeed`, `And a file "X" should exist`, `And "X" should contain "Y"`
- **Task 8 (graft add):** Implement steps for `add.feature` — reuses shared steps, adds `Given ... with "graft.toml"`, `And stderr should contain`
- **Task 10 (graft list):** Implement steps for `list.feature` — adds `Given a project with a synced graft` fixture step
- **Task 11 (graft check):** Implement steps for `check.feature` — adds `Then the exit code should be N`
- **Task 12 (graft outdated):** Implement steps for `outdated.feature` — adds upstream version fixture steps
- **Task 14 (graft upgrade):** Implement steps for `upgrade.feature` — adds merge-related fixture and assertion steps
- **Task 15 (graft resolve):** Implement steps for `resolve.feature`
- **Task 16 (graft remove):** Implement steps for `remove.feature`

Step definitions live in `tests/bdd.rs` (or split into `tests/bdd/` modules as they grow). Shared steps (directory setup, running commands, checking files) are defined once and reused across all features.

## Reference: File Tree (Final State)

```
~/repos/graft/
├── .github/
│   └── workflows/
│       └── ci.yml
├── .gitignore
├── CLAUDE.md
├── Cargo.toml
├── Cargo.lock
├── LICENSE
├── justfile
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs                # Library root: pub mod config, source, github, cache, checksum, merge, state, error
│   ├── cli.rs                # Clap derive definitions
│   ├── error.rs              # GraftError enum (thiserror + miette)
│   ├── cache.rs              # XDG cache layer (~/.graft/cache/)
│   ├── checksum.rs           # SHA-256 for files and directories
│   ├── merge.rs              # git merge-file wrapper
│   ├── state.rs              # GraftState enum + compute_state()
│   ├── config/
│   │   ├── mod.rs
│   │   ├── manifest.rs       # graft.toml types + parse + validate + save
│   │   └── lockfile.rs       # graft.lock types + parse + atomic save
│   ├── source/
│   │   ├── mod.rs
│   │   ├── parse.rs          # gh:owner/repo/path parsing
│   │   ├── version.rs        # SHA vs tag detection
│   │   └── tags.rs           # git ls-remote parsing, semver sorting
│   ├── github/
│   │   ├── mod.rs
│   │   ├── auth.rs           # Token resolution chain
│   │   └── client.rs         # GitHubClient: fetch_file, fetch_directory, resolve_ref, ls_remote_tags
│   └── commands/
│       ├── mod.rs
│       ├── init.rs
│       ├── add.rs
│       ├── sync.rs
│       ├── list.rs
│       ├── check.rs
│       ├── outdated.rs
│       ├── upgrade.rs
│       ├── resolve.rs
│       └── remove.rs
└── tests/
    ├── bdd.rs               # Cucumber BDD test runner + step definitions
    ├── features/
    │   ├── init.feature
    │   ├── add.feature
    │   ├── list.feature
    │   ├── check.feature
    │   ├── outdated.feature
    │   ├── upgrade.feature
    │   ├── resolve.feature
    │   └── remove.feature
    ├── init.rs              # Integration tests (supplement BDD)
    ├── add.rs
    ├── sync.rs
    ├── check.rs
    └── remove.rs
```

## Reference: Pre-Commit Checks

Every commit must pass these before being created:
```bash
cd ~/repos/graft && cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

## Reference: Commit Convention

All commits use conventional format. Never add co-authored-by lines.
- `chore:` — scaffolding, config, CI
- `feat:` — new functionality
- `fix:` — bug fixes
- `test:` — test-only changes
- `refactor:` — restructuring without behavior change
