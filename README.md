# graft

A version-tracked file dependency manager, written in Rust.

- **Track individual files** from any GitHub repo -Makefiles, CI workflows, linter configs, skills, whatever.
- **Three-way merge on upgrade.** Edit your local copy freely. When upstream updates, graft merges both sets of changes.
- **Manifest + lockfile.** `graft.toml` declares what you depend on. `graft.lock` pins exact commit SHAs and checksums.
- **Smart outdated detection.** Only reports a new version when the file content actually changed -not just when the tag bumped.
- **Works with private repos.** Uses your existing `GH_TOKEN`, `GITHUB_TOKEN`, or `gh` CLI credentials.
- **CI-friendly.** `graft check` exits non-zero if anything is modified or stale.
- **No publishing step.** Tag your repo. That's it.

## Install

```bash
cargo install --path .
```

Or grab a binary from [GitHub Releases](https://github.com/raiderrobert/graft/releases).

## Quick Start

```bash
graft init
graft add gh:your-org/shared-configs/workflows/ci.yml@v2.0.0 .github/workflows/ci.yml
```

This fetches the file, writes it locally, and records the source in `graft.toml`:

```toml
[deps.ci]
source = "gh:your-org/shared-configs/workflows/ci.yml"
version = "v2.0.0"
dest = ".github/workflows/ci.yml"
```

Later, check for updates and upgrade:

```bash
graft outdated        # see what's newer
graft upgrade         # pull updates, three-way merge if you've edited locally
```

Already have files you copied manually? Adopt them:

```bash
graft add gh:your-org/shared-configs/Makefile@v1.0.0 Makefile --adopt
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
