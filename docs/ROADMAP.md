# Graft Roadmap

## Shipped (v0.1)

- Single file and directory grafts from GitHub repos
- Three-way merge on upgrade
- Manifest (`graft.toml`) + lockfile (`graft.lock`)
- Smart outdated detection (content comparison, not just tag bumps)
- `--adopt` for tracking existing files
- `graft check` for CI
- Private repo support via existing GitHub credentials
- Optional `dest` argument (defaults to source path)
- Auto-create `graft.toml` on first `graft add`
- Install script

## Planned

### Bundles (multi-path groups)

A named group of files from different paths in the same source repo, versioned and upgraded as one unit.

**Motivating example:** release-please requires three files at different paths:
- `.github/workflows/release-please.yml`
- `release-please-config.json`
- `.release-please-manifest.json`

Today you need three separate `graft add` commands and they version independently. A bundle would let a source repo declare "these files go together" and consumers install/upgrade them as one.

**Possible manifest syntax:**

```toml
[bundles.release-please]
source = "gh:raiderrobert/graft-bundles/release-please"
version = "v1.0.0"

[[bundles.release-please.files]]
path = ".github/workflows/release-please.yml"
dest = ".github/workflows/release-please.yml"

[[bundles.release-please.files]]
path = "release-please-config.json"
dest = "release-please-config.json"

[[bundles.release-please.files]]
path = ".release-please-manifest.json"
dest = ".release-please-manifest.json"
```

Alternatively, the bundle definition could live in the source repo (a `graft-bundle.toml` alongside the files) so consumers just run `graft add gh:org/bundles/release-please@v1.0.0` and all three files are pulled in.

**Open questions:**
- Where does the bundle definition live? In the source repo (a `graft-bundle.toml`?) or in the consumer's manifest? Source-side is more ergonomic for consumers. Consumer-side is more flexible.
- How do partial upgrades work when one file in the bundle changed and two didn't? Probably upgrade all files to the same version regardless, since the whole point is they move together.
- What happens when a user wants different versions of files within a bundle? That means they don't want a bundle. They want individual grafts. Bundles are opinionated: same source, same version, same lifecycle.
- How does `--adopt` work with bundles? Probably: adopt all files that exist locally, fetch the rest.
- How does `graft remove` work? Remove the entire bundle and all its files from tracking? Or allow removing individual files from a bundle (which would break it into individual grafts)?
- Naming: "bundle" is the leading candidate. "kit" is the runner-up. Evaluated and rejected: scion (too obscure), suite (too heavy), constellation (too long), pallet (too niche), pack (too close to "package").

**Decision:** Wait for real user demand before building. Use naming prefixes (`release-please/workflow`, `release-please/config`) as a convention in the meantime. The devil's advocate argument is strong: every tool that added grouping early spent the rest of its life managing the complexity. npm didn't ship workspaces until v7. Start with the primitive.

### Version constraints / ranges

Semver range operators (`^1.2.0`, `~1.2.0`) so `graft sync` can resolve the latest matching version within a constraint automatically.

**How it would work:**
- `version = "^1.2.0"` in `graft.toml` means "any 1.x.y >= 1.2.0"
- `version = "~1.2.0"` means "any 1.2.x >= 1.2.0"
- `version = "v1.2.0"` (bare, no operator) continues to mean "exactly this version" for backwards compatibility
- `graft sync` resolves the latest matching version and records the exact version in `graft.lock`
- A new `graft update` command would re-resolve within existing constraints (update the lockfile without changing the manifest), matching cargo's `cargo update`
- The existing `graft upgrade` would bump the constraint itself in `graft.toml` (like `cargo upgrade` from cargo-edit)

**Why this matters:** Without ranges, every upstream bugfix requires manually editing `graft.toml`. With ranges, `graft sync` picks up patch releases automatically, and `graft upgrade` is reserved for intentional major/minor bumps.

**Complexity cost:** Version resolution logic, constraint parsing (use the `semver` crate's `VersionReq`), and a third command (`graft update`) that needs clear differentiation from `graft upgrade`. The three-command model (outdated/update/upgrade) mirrors cargo but is confusing for new users.

### Parameterization

Files that need per-project values on fetch. For example, a `release-please-config.json` that needs the project's current version number, or a CI workflow that needs the project's Node version.

**Today's workaround:** Edit the file locally after fetching. The three-way merge preserves your edits on upgrade. This works well when the edits are small and infrequent.

**When it breaks down:** When every consumer of a file makes the same kind of edit (changing one value), the three-way merge works but feels like unnecessary friction. You fetch, edit, commit. Every upgrade merges cleanly but you're merging around the same substitution every time.

**Possible approach:** A `vars` block in the manifest:

```toml
[deps.ci]
source = "gh:org/shared-configs/workflows/ci.yml"
version = "v2.0.0"
dest = ".github/workflows/ci.yml"

[deps.ci.vars]
node_version = "20"
deploy_target = "production"
```

The upstream file would use Tera template syntax (`{{ node_version }}`). Graft renders on fetch, stores the rendered result. On upgrade, graft re-renders the new upstream version with the same vars before merging.

**Risk:** This slides toward diecut territory. Diecut is the project template generator; graft is the file dependency manager. Adding a template engine blurs the line. The constraint should be: parameterization is for simple value substitution, not conditional logic, loops, or file generation.

**Decision:** Deliberately scoped out of v1. Monitor how often people eject/modify files just to change one value. If the pattern is common, add it.

### Sub-file granularity

Section-level grafting for files where you want to pin shared sections but own the rest of the file.

**Motivating example:** A `CLAUDE.md` that is 80% shared boilerplate (commit conventions, pre-commit checks, testing philosophy) but needs project-specific sections (tech stack, file paths, project-specific rules).

**Possible approach:** Marker comments that define graftable regions:

```markdown
<!-- graft:start deps.coding-standards -->
## Coding Standards
Use conventional commits. Run pre-commit checks before committing.
<!-- graft:end -->

## Project-Specific
This section is mine and graft won't touch it.
```

On upgrade, only the content between markers is merged/replaced. Everything outside the markers is untouched.

**Challenges:**
- Marker syntax needs to work across file types (HTML comments for markdown, `# graft:start` for YAML/TOML, `// graft:start` for JSON with comments). Or just pick one format and accept it won't look native everywhere.
- Three-way merge within a section is more complex than whole-file merge. Need to extract the section, merge it, and splice it back in.
- What happens when someone edits inside the markers? Same three-way merge as today, just scoped to the section.
- What happens when the upstream adds content outside the markers? That content is ignored by graft since it's outside the graftable region.
- JSON and TOML configs are the files where this is most wanted, but they're also the files where marker comments are least natural. A structural merge approach (deep-merge keys in JSON/TOML) might be better for config files specifically.

**Decision:** Out of scope for now. The marker approach is the simplest path but needs more design work. A separate `merge = "json-deep"` strategy per graft might be more practical for config files.

### Non-GitHub sources

v1 is GitHub-only. The source resolution layer (`gh:owner/repo/path`) is designed to be extensible.

**Planned sources:**
- `gl:owner/repo/path` for GitLab (same abbreviation pattern as diecut)
- `cb:owner/repo/path` for Codeberg
- Full HTTPS git URLs for self-hosted instances (`https://git.company.com/org/repo/path`)
- Local file paths for development/testing

**Implementation approach:** The `GraftSource` struct already separates parsing from fetching. Adding a new source means:
1. Add a new prefix to the parser (`gl:`, `cb:`, or URL detection)
2. Implement a new `fetch_file` / `fetch_directory` backend for that host's API
3. Implement `ls_remote_tags` (this already uses `git ls-remote` which works with any git host)

**Authentication per host:** Each host needs its own token resolution. GitLab uses `GITLAB_TOKEN`, Codeberg uses personal access tokens. The auth module would need a host-to-token mapping, possibly via a config file (`~/.graft/config.toml`).

**Priority:** Depends on user demand. If most users are on GitHub, this can wait. If someone needs GitLab support, the abstraction is ready.

### Push-based automation

A reference GitHub Action that runs `graft outdated` or `graft check` on a schedule and opens issues or PRs when grafts are stale.

**Why it matters:** Graft today is pull-only. You have to remember to run `graft outdated`. Nobody will remember to do this across 30 repos. Renovate's killer feature is that PRs appear automatically. Without push-based automation, graft remains a personal productivity tool.

**Approach: GitHub Action**

```yaml
# .github/workflows/graft-check.yml
name: Graft Check
on:
  schedule:
    - cron: '0 9 * * 1'  # weekly on Monday at 9am
  workflow_dispatch:

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: raiderrobert/graft-action@v1
        with:
          command: outdated
          create-issue: true  # or create-pr: true
```

**Two modes:**
1. **Issue mode:** `graft outdated` runs, if anything is outdated, opens a GitHub issue listing what's available. Low friction, no code changes.
2. **PR mode:** `graft upgrade --all` runs, commits the changes, opens a PR. Higher value but requires the action to have write permissions and handle merge conflicts.

**Scope:** The action itself is a separate repo (`raiderrobert/graft-action`). Graft the CLI just needs to work in CI, which it already does. The action is a wrapper.

**Priority:** High value, relatively low effort. This is probably the single highest-leverage feature for adoption beyond personal use.

### Registry / discovery

Today there's no way to discover what files are available to graft. You find them through READMEs, word of mouth, or GitHub search.

**Possible approaches:**
- A curated `awesome-grafts` repo listing useful graft sources
- A `graft search` command that searches GitHub for repos containing graftable files (by convention, repos with a `graft-bundle.toml` or a `grafts/` directory)
- A web registry (like crates.io or npm) where people publish graft sources with descriptions and categories

**Decision:** Not planned for the near term. The "tag your repo, that's it" philosophy is a feature. A registry adds governance, moderation, and supply chain concerns. Start with an awesome-list if discovery becomes a problem.

### Conflict resolution UI

Today conflicts produce standard git conflict markers and you resolve them in your editor. This is fine for developers but not great for non-technical users or for large files with many conflicts.

**Possible improvements:**
- `graft diff <name>` to preview incoming changes before upgrading (partially addressed by `--dry-run`)
- `graft upgrade --theirs <name>` to accept all upstream changes (discard local edits)
- `graft upgrade --ours <name>` to keep all local changes (skip upstream, but update the version)
- Integration with `git mergetool` for interactive conflict resolution

**Priority:** Low. The current git conflict marker approach works for the target audience (developers). The `--theirs` / `--ours` shortcuts would be the highest-value addition here.

### Shared cache with diecut

Both graft and diecut fetch content from GitHub repos and cache it locally. Today they have separate caches (`~/.graft/cache/` and `~/.cache/diecut/templates/`). A shared cache crate (backed by something like `cacache`) could deduplicate fetched content across both tools.

**Priority:** Low. The caches are small and the overlap is minimal in practice. Worth doing if both tools see significant adoption and the duplicate downloads become noticeable.
