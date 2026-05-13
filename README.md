# bgg-cli

A single-purpose CLI that fetches a BoardGameGeek user's collection, caches it
locally, and keeps the cache in sync. One-way (BGG → local). Single user.

## Install

```
cargo install --path .
```

## Use

```
bgg                        # default: status (auth state, item count, last sync)
bgg auth                   # prompt for username + password, store cookies in keyring
bgg auth --clear           # remove stored cookies
bgg sync                   # incremental sync
bgg sync --full            # full sync (required to detect deletions)
bgg list                   # table of owned base games
bgg list --sort=plays      # sort by plays (also: name, year, bggid, rating, time, added, geek, players)
bgg list --sort=-rating    # prefix `-` inverts the natural direction
bgg list --cols=all        # show every column; or e.g. --cols=year,name,time
bgg list --json | jq ...   # full unfiltered collection as JSON
bgg status                 # explicit status (same as `bgg`)
```

Cookies live in the OS keyring (Secret Service / macOS Keychain / Windows
Credential Manager). Headless Linux boxes without a running Secret Service
are not supported in v1.

The cache lives at `$XDG_STATE_HOME/bgg-cli/collection-<username>.json` and is
plain JSON — for one-off lookups, just point `jq` at it.

## Status

Alpha. Read the spec at
[`docs/superpowers/specs/2026-05-13-bgg-cli-scaffold-design.md`](docs/superpowers/specs/2026-05-13-bgg-cli-scaffold-design.md).
