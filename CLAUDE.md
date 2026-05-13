# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Status

Fresh repo. No code yet. This file captures the intended scope and the decisions made before any code was written, so future sessions don't re-litigate them.

## Project intent

`bgg-cli` is a single-purpose CLI: **fetch a BoardGameGeek user's collection, cache it locally, and keep the cache in sync**. That is the entire product. No search, no stashes, no profiles, no discovery features.

Sync is **one-way (BGG → local)**. Pushing edits back to BGG is out of scope — those endpoints are undocumented and form-based on the website.

## Key technical decisions

- **Language:** Rust.
- **Auth:** Cookie-based via BGG's JSON login endpoint (`POST https://boardgamegeek.com/login/api/v1` with `{"credentials":{"username":..., "password":...}}`). This returns `bggusername` / `bggpassword` / `SessionID` cookies that satisfy the `xmlapi2/collection` auth check. No app registration, no Bearer token, no "Powered by BGG" obligation.
  - We confirmed empirically that anonymous `xmlapi2/collection` returns `401 Unauthorized` with `WWW-Authenticate: Bearer realm="xml api"`, so auth is mandatory.
- **Credential storage:** OS keyring via the `keyring` crate (Secret Service / macOS Keychain / Windows Credential Manager). Store the **cookies**, not the password. On 401, prompt for password and re-login.
  - **Fallback:** headless Linux often has no Secret Service running. Need a fallback (encrypted file or plaintext with a loud warning) before this is usable on servers/CI.
- **The only network call:** `GET https://boardgamegeek.com/xmlapi2/collection?username=<name>&stats=1`. Use `modifiedsince=YY-MM-DD` for incremental sync. Handle `202 Accepted` by retrying with backoff (BGG queues collection requests).
- **Rate limiting:** BGG asks for a 5-second minimum between requests. With one call per sync this is mostly moot, but enforce it if we ever batch.
- **Local cache:** TBD format. RON or JSON are both reasonable; pick whichever serializes the collection-item shape cleanly. Cache lives under `$XDG_STATE_HOME/bgg-cli/` (or platform equivalent).

## Reference / prior art

`~/dev/mpl-cli` is the previous attempt at a broader BGG CLI by another author that I was hacking on. It has reusable scaffolding worth looking at (not copying wholesale):

- `src/cli.rs` — clap-derive layout patterns.
- `src/util/fs.rs` — XDG path resolution per OS, `check_fs()` bootstrap.
- `src/util/bgg_api.rs` — blocking `reqwest` with 202 retry loop (but no auth, no rate limit).
- `src/util/xml.rs` + `src/structs/title.rs` — `xmltree`-based parsing of the `thing` endpoint. Note: the **collection** endpoint XML shape is different and flatter — don't assume `Title::from` carries over.
- `docs/bgg-api.md` — full endpoint reference for the BGG XML API v2, including the `collection` params, auth requirements, and rate limits. **Read this before writing any HTTP code.**

The mpl-cli profile/stash/multi-everything model is **deliberately not carried over** — this app is single-user, single-collection.
