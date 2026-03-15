# graft

You copied that GitHub Actions workflow from your other repo six months ago. You've tweaked it since. Now the original has a fix you need — but you can't just overwrite your copy because you'll lose your changes.

Graft tracks where files came from and merges upstream updates with your local modifications.

```
graft add gh:your-org/shared-configs/workflows/ci.yml@v2.1.0 .github/workflows/ci.yml
```

That's it. Graft fetches the file, records the source and version in `graft.toml`, and locks the exact commit SHA in `graft.lock`. Both files go into your repo.

Six months later when upstream ships a fix:

```
$ graft outdated
ci  v2.1.0 → v2.3.0  .github/workflows/ci.yml

$ graft upgrade ci
Merged ci (v2.1.0 → v2.3.0) — clean merge, your changes preserved.
```

Graft does a three-way merge: the version you originally fetched, your current file, and the new upstream version. Your local tweaks stay. Upstream fixes land. Conflicts get standard git conflict markers — resolve them the way you already know how.

## Install

```bash
cargo install --path .
```

Or grab a binary from [GitHub Releases](https://github.com/raiderrobert/graft/releases).

## Usage

```bash
graft init                                              # create graft.toml
graft add gh:owner/repo/path@v1.0.0 local/path          # fetch a file
graft add gh:owner/repo/path@v1.0.0 local/path --adopt  # track a file you already have
graft list                                               # see what you're tracking
graft outdated                                           # check for newer versions
graft upgrade                                            # pull updates with merge
graft check                                              # exit 1 if anything is stale (for CI)
```

Works with private repos — graft uses your existing `GH_TOKEN`, `GITHUB_TOKEN`, or `gh` CLI credentials.

## The manifest

```toml
# graft.toml — you edit this
[deps.ci]
source = "gh:your-org/shared-configs/workflows/ci.yml"
version = "v2.1.0"
dest = ".github/workflows/ci.yml"

[deps.makefile]
source = "gh:your-org/shared-configs/Makefile"
version = "v1.4.0"
dest = "Makefile"
```

`graft.lock` is auto-generated — commit SHA, file checksum, never hand-edited. Both files get checked in.

## License

[PolyForm Shield 1.0.0](LICENSE)
