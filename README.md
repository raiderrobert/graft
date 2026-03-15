# graft

A "package manager" for paths, both files and dirs.

- **Pin files, not packages.** Pull a Makefile, a CI workflow, or a linter config from any GitHub repo and lock it to a version.
- **Edit freely, upgrade safely.** Your local changes are preserved through three-way merge on upgrade.
- **No registry, no publishing.** Tag your source repo. Any file in any GitHub repo is a valid dependency.
- **CI-ready.** `graft check` exits non-zero if anything is modified or stale.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/raiderrobert/graft/main/install.sh | sh
```

Or build from source:

```bash
cargo install --path .
```

Or grab a binary from [GitHub Releases](https://github.com/raiderrobert/graft/releases).

## Quick Start

```bash
graft init
graft add gh:raiderrobert/graft/justfile@v0.1.0 justfile
```

This fetches the file, writes it locally, and records the source in `graft.toml`:

```toml
[deps.justfile]
source = "gh:raiderrobert/graft/justfile"
version = "v0.1.0"
dest = "justfile"
```

Later, check for updates and upgrade:

```bash
graft outdated        # see what's newer
graft upgrade         # pull updates, three-way merge if you've edited locally
```

Already have a file you copied manually? Adopt it:

```bash
graft add gh:raiderrobert/graft/justfile@v0.1.0 justfile --adopt
```

This tracks the file without overwriting your local version. The next `graft upgrade` will merge upstream changes with your edits.

## Commands

```
graft init                  Create graft.toml
graft add <src@ver> <dest>  Fetch and track a file
graft add ... --adopt       Track an existing local file
graft sync                  Fetch all dependencies
graft list                  Show grafts with status
graft check                 Verify all clean (for CI)
graft outdated              Show newer upstream versions
graft upgrade [name]        Upgrade with three-way merge
graft upgrade --dry-run     Preview changes
graft resolve <name>        Mark conflicts as resolved
graft remove <name>         Stop tracking (keeps file)
```

## License

[PolyForm Shield 1.0.0](LICENSE)
