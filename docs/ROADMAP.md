# Graft Roadmap

## Shipped (v0.1)

- Single file and directory grafts from GitHub repos
- Three-way merge on upgrade
- Manifest (`graft.toml`) + lockfile (`graft.lock`)
- Smart outdated detection (content comparison, not just tag bumps)
- `--adopt` for tracking existing files
- `graft check` for CI
- Private repo support via existing GitHub credentials
- Install script

## Planned

### Bundles (multi-path groups)

A named group of files from different paths in the same source repo, versioned and upgraded as one unit.

**Motivating example:** release-please requires three files at different paths:
- `.github/workflows/release-please.yml`
- `release-please-config.json`
- `.release-please-manifest.json`

Today you need three separate `graft add` commands and they version independently. A bundle would let a source repo declare "these files go together" and consumers install/upgrade them as one.

**Open questions:**
- Where does the bundle definition live? In the source repo (a `graft-bundle.toml`?) or in the consumer's manifest?
- How do partial upgrades work when one file in the bundle changed and two didn't?
- What happens when a user wants different versions of files within a bundle?
- Naming: "bundle" is the leading candidate. "kit" is the runner-up. Evaluated: scion, suite, constellation, pallet, pack, and others.

**Decision:** Wait for real user demand before building. Use naming prefixes (`release-please/workflow`, `release-please/config`) as a convention in the meantime.

### Version constraints / ranges

Semver range operators (`^1.2.0`, `~1.2.0`) so `graft sync` can resolve the latest matching version automatically.

Would enable a `graft update` command (resolve within constraints) alongside the existing `graft upgrade` (bump beyond constraints), matching the cargo model.

The `version` field is designed for backwards compatibility: a bare version like `v1.2.0` will continue to mean "exactly this version" when ranges are added.

### Parameterization

Files that need per-project values (a version number, a project name, a path). Today you edit locally and the three-way merge preserves your edits on upgrade. If the "always editing the same fields" pattern is common enough, light parameterization (variables in the manifest, Tera rendering on fetch) could eliminate that friction.

Deliberately scoped out of v1 to avoid sliding into diecut territory.

### Sub-file granularity

Section-level grafting for files like CLAUDE.md where you want to pin shared sections but own the rest. Something like marker comments (`<!-- graft:start -->` / `<!-- graft:end -->`) to define graftable regions within a file.

### Non-GitHub sources

GitLab, Bitbucket, self-hosted Gitea, plain git URLs. v1 is GitHub-only. The source resolution layer is designed to be extensible.

### Push-based automation

A reference GitHub Action that runs `graft outdated` or `graft check` on a schedule and opens issues or PRs when grafts are stale. Bridges the gap between "personal tool" and "team tool."
