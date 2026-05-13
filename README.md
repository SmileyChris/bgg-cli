# bgg-cli

A single-purpose CLI that fetches a BoardGameGeek user's collection, caches it
locally, and keeps the cache in sync. One-way (BGG → local). Single user.

## Install

```
cargo install --path .
```

## Use

```
bgg                        # one-screen summary: counts, plays, ratings, last sync
bgg auth                   # prompt for username + password, store credentials in keyring
bgg auth --clear           # remove stored credentials
bgg sync                   # incremental sync
bgg sync --full            # full sync (required to detect deletions)
bgg list                   # table of owned base games (default --filter owned,not:expansion)
bgg list --filter wishlist # filter to wishlist; also prev-owned, want-to-play, want-to-buy, preordered, for-trade, expansion, rated, played, solo, all
bgg list --filter owned,solo,not:played  # AND-combined; prefix `not:` to invert
bgg list --sort=plays      # sort by plays (also: name, year, bggid, rating, time, added, geek, players)
bgg list --sort rating:asc # append :asc or :desc to override the natural direction
bgg list --cols=all        # show every column; or e.g. --cols=year,name,time
bgg list --limit 10        # cap rows; footer shows "N of M items" in TTY
bgg list --json | jq ...   # full unfiltered collection as JSON
bgg stats                  # full breakdown: counts, plays, ratings, year/time/players
bgg stats --json           # same numbers as a JSON object for scripting
```

Credentials (password + cookies + session expiry) live in the OS keyring
(Secret Service / macOS Keychain / Windows Credential Manager). The cookie
session expires after about an hour; `bgg sync` refreshes it silently using
the stored password. Headless Linux boxes without a running Secret Service
are not supported in v1.

The cache lives at `$XDG_STATE_HOME/bgg-cli/collection-<username>.json` and is
plain JSON — for one-off lookups, just point `jq` at it.

## Status

Alpha. Read the spec at
[`docs/superpowers/specs/2026-05-13-bgg-cli-scaffold-design.md`](docs/superpowers/specs/2026-05-13-bgg-cli-scaffold-design.md).
