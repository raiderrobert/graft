# Graft

Rust CLI tool — a package manager for config files. Manages versioned file dependencies from GitHub repos with three-way merge on upgrade.

## Project Structure

Single crate with both library (`src/lib.rs`) and binary (`src/main.rs`) targets.
- `src/lib.rs` — library root
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
- `tests/` — integration tests and BDD feature files

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
- BDD tests use cucumber crate with .feature files in `tests/features/`
- Conventional commits (feat:, fix:, chore:, etc.)
- NEVER add co-authored-by lines for Claude/AI
