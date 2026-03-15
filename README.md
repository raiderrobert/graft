# graft

A package manager for config files, written in Rust.

Graft manages versioned dependencies on individual files and directories from GitHub repos. Declare what you depend on in `graft.toml`, fetch with `graft sync`, and upgrade with three-way merge that preserves your local modifications.

## Install

Build from source:

```bash
cargo install --path .
```

Or grab a binary from [GitHub Releases](https://github.com/raiderrobert/graft/releases).

## Quick Start

```bash
# Initialize a project
graft init

# Add a file from a GitHub repo
graft add gh:org/shared-configs/Makefile@v1.0.0 Makefile

# Add a directory of files
graft add gh:org/shared-configs/workflows/@v1.0.0 .github/workflows/

# Track a file you already copied manually
graft add gh:org/shared-configs/Makefile@v1.0.0 Makefile --adopt

# Check status of all grafts
graft list

# Check for newer upstream versions
graft outdated

# Upgrade (three-way merge if you've made local changes)
graft upgrade

# Verify everything is in sync (for CI)
graft check
```

## How It Works

**`graft.toml`** declares what you depend on:

```toml
[deps.lint]
source = "gh:org/shared-configs/workflows/lint.yml"
version = "v1.2.0"
dest = ".github/workflows/lint.yml"

[deps.makefile]
source = "gh:org/shared-configs/Makefile"
version = "v1.2.0"
dest = "Makefile"
```

**`graft.lock`** records the exact resolved state (commit SHAs, checksums). Both files are checked into your repo.

When you upgrade, graft does a **three-way merge** — your local modifications are preserved, upstream changes are applied, and conflicts get standard git conflict markers.

## Commands

| Command | Description |
|---|---|
| `graft init` | Create an empty `graft.toml` |
| `graft add <source@version> <dest>` | Add a file dependency |
| `graft add ... --adopt` | Track an existing local file |
| `graft sync` | Fetch all dependencies |
| `graft list` | Show all grafts with status |
| `graft check` | Verify all grafts are clean (exit code for CI) |
| `graft outdated` | Show grafts with newer upstream versions |
| `graft upgrade [name]` | Upgrade to newer versions |
| `graft upgrade --dry-run` | Preview what would change |
| `graft resolve <name>` | Mark a conflicted graft as resolved |
| `graft remove <name>` | Remove a graft (keeps local file) |

## Authentication

Graft uses your existing GitHub credentials. It checks, in order:

1. `GH_TOKEN` environment variable
2. `GITHUB_TOKEN` environment variable
3. `gh auth token` (GitHub CLI)

Private repos work transparently if you have credentials configured.

## License

[PolyForm Shield 1.0.0](LICENSE)
