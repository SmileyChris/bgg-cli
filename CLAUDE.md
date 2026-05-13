# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Status

Scaffold v0.1 shipped on `main` (2026-05-13). Commands: `bgg auth [--clear]`,
`bgg sync [--full]`, `bgg list [--owned] [--json]`, `bgg stats [--json]`,
`bgg status` (default).
End-to-end tested against real BGG: 1174-item collection round-trips.

Module layout, design rationale, and deferred work live in the spec at
`docs/superpowers/specs/2026-05-13-bgg-cli-scaffold-design.md`. The build plan
at `docs/superpowers/plans/2026-05-13-bgg-cli-scaffold.md` is now history.

This file captures durable decisions that should not be re-litigated.

## Project intent

`bgg-cli` is a single-purpose CLI: **fetch a BoardGameGeek user's collection, cache it locally, and keep the cache in sync**. That is the entire product. No search, no stashes, no profiles, no discovery features.

Sync is **one-way (BGG → local)**. Pushing edits back to BGG is out of scope — those endpoints are undocumented and form-based on the website.

## Key technical decisions

- **Language:** Rust.
- **Auth:** Cookie-based via BGG's JSON login endpoint (`POST https://boardgamegeek.com/login/api/v1` with `{"credentials":{"username":..., "password":...}}`). Returns `bggusername` / `bggpassword` / `SessionID` cookies (HTTP 204). No app registration, no Bearer token, no "Powered by BGG" obligation.
  - **Verified empirically 2026-05-13:**
    - Anonymous calls to every XML endpoint return `401` with `WWW-Authenticate: Bearer realm="xml api"`.
    - The login cookies authorize **only user-specific reads**: `xmlapi/collection/<user>` (v1, 202→200), `xmlapi2/collection?username=<user>` (v2, 202→200), and `xmlapi2/plays?username=<user>` (v2, 200). Every other endpoint we swept — `xmlapi2/thing`, `xmlapi2/search`, `xmlapi2/hot`, `xmlapi2/user`, `xmlapi2/family`, `xmlapi2/forum`, `xmlapi2/forumlist`, `xmlapi2/thread`, `xmlapi2/guild`, plus the v1 `boardgame`, `search`, `thread`, `geeklist` — returns 401 with cookies. Those endpoints require a registered application and Bearer token.
    - The 202 → retry pattern is real: first call to `collection` returned 202 with a "queued" message; immediate retry returned 200 with full XML.
    - Implication for scope: cookie auth covers `collection` (our entire current scope) and would also cover a future "sync my plays" feature. Anything else — `thing` lookups, search, hotness — is gated behind app registration and out of reach without a different auth path.
- **Credential storage:** OS keyring via the `keyring` crate. We store **password + cookies + session-expiry** in a single per-user blob (`StoredCreds`).
  - **Why the password is stored** (the original spec said cookies-only): the `SessionID` cookie has `Max-Age=3600`, so cookie-only auth forces a password re-prompt every hour of active use. Storing the password lets `bgg sync` refresh cookies silently — proactively when the recorded `session_fresh_until` has passed, reactively on a server 401 as a safety net. The keyring threat model is the same either way (a compromised keyring already leaks `bggpassword`, which is effectively a long-lived auth token).
  - On stored-password refresh failure (`BadCredentials` from `/login/api/v1`), surface "stored password no longer works — run `bgg auth`".
  - **`keyring` v3 requires explicit backend features** — defaults to a per-process in-memory mock that silently loses data across runs. Our `Cargo.toml` enables `apple-native`, `windows-native`, `sync-secret-service`, `crypto-rust`. On Fedora the `sync-secret-service` backend needs `dbus-devel` + `pkgconf-pkg-config` at build time.
  - **Headless Linux without Secret Service is unsupported in v1.** Encrypted-file fallback was scoped out as not needed; revisit if it becomes a real constraint.
- **Network calls per sync:** `GET https://boardgamegeek.com/xmlapi2/collection?username=<name>&stats=1&subtype=<X>`, once with `subtype=boardgame` and once with `subtype=boardgameexpansion`. BGG's collection endpoint only returns one subtype per request — without an explicit subtype, the response is **silently mis-tagged**: every item comes back with `subtype="boardgame"` regardless of its true type, so the only way to distinguish base games from expansions is to ask for each subtype explicitly. Use `modifiedsince=YY-MM-DD HH:MM:SS` (with a 1-minute safety margin) for incremental sync. Handle `202 Accepted` by retrying with backoff.
- **Rate limiting:** BGG asks for a 5-second minimum between requests. Enforced in `HttpClient` via a `Mutex<Instant>`; the two subtype calls in a sync are naturally spaced past this floor.
- **Local cache:** JSON at `$XDG_STATE_HOME/bgg-cli/collection-<username>.json`. Plain JSON so `jq` covers ad-hoc lookups — that's why there's no `bgg show` subcommand.
  - **Cache items are keyed by `collid`, not BGG `objectid`.** BGG's collection XML can return multiple `<item>` elements with the same `objectid` (a user who owns two printings of the same game), each with a distinct `collid`. Keying by `objectid` silently collapses those duplicates. `collid` is the per-collection-entry unique id and is the right primary key.

## Build / test notes

- `cargo build`, `cargo test`, `cargo fmt`. `cargo clippy` not installed on the
  dev box — add via `dnf install rust-clippy` if you want lint runs.
- System deps for keyring's `sync-secret-service` backend on Fedora:
  `sudo dnf install dbus-devel pkgconf-pkg-config`.
- Wiremock integration tests pull in `tokio` with `rt-multi-thread` — required,
  not just `rt`.

## Reference / prior art

`~/dev/mpl-cli` is a previous broader BGG CLI by another author. Mostly
superseded now; the one still-useful artifact is `docs/bgg-api.md`, a full
endpoint reference for the XML API. **Cross-check it against this file's
"Verified empirically" section** — anonymous-readable claims in that doc are
out of date (everything 401s anonymously now).

The mpl-cli profile/stash/multi-everything model is **deliberately not carried
over** — this app is single-user, single-collection.
