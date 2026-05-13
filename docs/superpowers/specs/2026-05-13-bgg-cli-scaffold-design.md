# bgg-cli scaffold — design

Initial scaffold for `bgg-cli`. Covers CLI surface, module layout, crate choices,
on-disk layout, and the auth/sync data flow. Keep CLAUDE.md as the source of
truth for product scope; this doc is the implementation shape.

## Goals

- Single-purpose CLI: fetch a BGG user's collection, cache it, keep it in sync.
- One-way sync only (BGG → local).
- Auth via BGG's JSON login endpoint; store cookies in OS keyring with an
  encrypted-file fallback for headless boxes.
- Be obvious to read. Few modules, narrow responsibilities, small files.

## Non-goals

- Pushing edits back to BGG.
- Multi-user / multi-collection / profiles / stashes.
- Search, hotness, plays, geeklists, anything outside `xmlapi2/collection`.
- A daemon, watcher, or background sync.

## CLI surface

Single binary, name `bgg`. Subcommands:

| Command            | Purpose                                                      |
| ------------------ | ------------------------------------------------------------ |
| `bgg login`        | Prompt for username + password; fetch & store cookies.       |
| `bgg logout`       | Clear stored cookies (and cached username).                  |
| `bgg sync`         | Incremental sync via `modifiedsince`. Default command.       |
| `bgg sync --full`  | Ignore `modifiedsince`. Required to detect deletions.        |
| `bgg list`         | Print the cached collection. Flags: `--owned`, `--json`.     |
| `bgg show <id>`    | Print one cached item by BGG ID.                             |
| `bgg status`       | Show username, last sync time, item count, auth state.       |

Global flags: `--verbose / -v`, `--cache-dir <path>` (override XDG).

Exit codes: `0` ok, `1` generic error, `2` auth required (re-run `login`).

## Module layout

```
src/
  main.rs          // entrypoint: parse cli, dispatch
  cli.rs           // clap-derive structs
  cmd/
    mod.rs
    login.rs
    logout.rs
    sync.rs
    list.rs
    show.rs
    status.rs
  auth.rs          // login flow, cookie jar (de)serialization
  secrets.rs       // keyring + encrypted-file fallback
  bgg/
    mod.rs         // public client API
    client.rs      // reqwest blocking, cookie header, 202 retry, rate gate
    collection.rs  // GET /xmlapi2/collection params + response handling
    parse.rs       // XML → CollectionItem (quick-xml)
  cache.rs         // load/save collection cache, merge logic
  paths.rs         // XDG resolution via `directories` crate
  model.rs         // CollectionItem, CacheFile, Cookies, etc.
  error.rs         // thiserror types; anyhow at command boundaries
```

Each file stays small (target < 300 lines). `bgg/` is the only module that
touches the network. `cmd/*` files orchestrate; they don't parse XML or build
HTTP requests directly.

## Crate choices

- `clap` (derive, cargo features) — CLI.
- `reqwest` (`blocking`, `cookies`, `rustls-tls`) — HTTP.
- `serde`, `serde_json` — login request body, cache file, cookie blob.
- `quick-xml` (with `serialize`) — collection XML parsing. Cleaner than
  `xmltree` for the flatter v2 collection shape and faster.
- `keyring` — primary secret store.
- `directories` — XDG/macOS/Windows paths (don't roll our own).
- `chrono` (serde) — timestamps for `modifiedsince` and `last_sync`.
- `rpassword` — hidden password prompt.
- `aes-gcm` + `argon2` — encrypted fallback when keyring is unavailable.
- `thiserror`, `anyhow` — errors.
- `tracing` + `tracing-subscriber` — logging behind `-v`.

Dev-only: `assert_cmd`, `wiremock` (or `mockito`) for HTTP fixtures, `insta`
for snapshot tests on XML parsing.

## On-disk layout

Paths via `directories::ProjectDirs("", "", "bgg-cli")`:

```
$XDG_STATE_HOME/bgg-cli/
  collection-<username>.json     // cache: header + items map
$XDG_DATA_HOME/bgg-cli/
  cookies.enc                    // encrypted fallback (only if keyring fails)
$XDG_CONFIG_HOME/bgg-cli/
  config.toml                    // optional; for now just { username = "..." }
```

Username is the join key across all three. `config.toml` exists so `bgg sync`
with no args knows whose collection to fetch without keyring access.

### Cache file shape

```json
{
  "username": "chris",
  "last_sync": "2026-05-13T19:04:21Z",
  "items": {
    "174430": { "id": 174430, "name": "Gloomhaven", "own": true, "...": "..." }
  }
}
```

Keyed by BGG ID as a string (JSON object keys). Incremental sync replaces
matching keys; it cannot detect deletions — `--full` is the only way to prune.
This limitation is documented in `bgg status` output.

## Auth & secrets

**Login flow** (`bgg login`):

1. Prompt username (default to cached) and password (`rpassword`).
2. `POST https://boardgamegeek.com/login/api/v1` with
   `{"credentials":{"username":..,"password":..}}`.
3. On 200, extract `bggusername`, `bggpassword`, `SessionID` cookies from the
   response.
4. Serialize the three cookies (name, value, expiry) as JSON.
5. Store via `secrets::store(username, blob)`:
   - Try `keyring::Entry::new("bgg-cli", &username).set_password(&blob)`.
   - On keyring error (no Secret Service, etc.), prompt:
     "Keyring unavailable. Store cookies in an encrypted file? [y/N]"
     - Yes: prompt for a passphrase, derive key via Argon2, AES-GCM-encrypt
       the blob, write `cookies.enc` (0600).
     - No: abort with instructions.
6. Write `config.toml` with the username.

**Use flow** (`bgg sync`):

1. Load username from `config.toml`.
2. Load cookie blob via `secrets::load(username)` (keyring → fallback file,
   prompting for the encrypted-file passphrase if needed).
3. Build `Cookie:` header, attach to `reqwest::Client`.
4. Call collection endpoint.
5. On `401`, exit code 2 with "run `bgg login`".

## Sync data flow

```
cmd::sync ─► cache::load ──► last_sync timestamp
            └► bgg::collection::fetch(username, modifiedsince) ──► 200 XML
                                                              │
                                                              └► 202 retry (12s, max ~5min)
            └► bgg::parse::items(xml) ──► Vec<CollectionItem>
            └► cache::merge(existing, new) ──► CacheFile
            └► cache::save
            └► print "synced N items (M new, K updated)"
```

`bgg::client` enforces a 5-second floor between calls via a `Mutex<Instant>`
even though we only make one call per sync — cheap insurance for future
batching.

`modifiedsince` format: `YY-MM-DD%20HH:MM:SS`, derived from the cache's
`last_sync` minus a 1-minute safety margin.

## Error handling

- `error::Error` (thiserror) for typed cases: `AuthRequired`, `RateLimited`,
  `Queued`, `Network(reqwest::Error)`, `Parse(...)`, `Cache(...)`,
  `Secrets(...)`.
- `cmd/*` returns `anyhow::Result<()>`. `main` maps known errors to exit codes
  and a single-line user-friendly message; uses `{:?}` only under `-v`.

## Testing strategy

- **XML parsing**: snapshot tests (`insta`) against fixtures in
  `tests/fixtures/collection/*.xml`. Capture at least: empty collection,
  owned-only, wishlist with priority, expansion, 202 placeholder.
- **HTTP client**: `wiremock` server returns canned 202→200 sequences and 401s.
- **Cache merge**: pure-function unit tests on `cache::merge`.
- **CLI integration**: `assert_cmd` smoke tests for `--help`, `status` with no
  cache, `list --json` against a seeded cache.
- **Auth**: unit-test the secrets module against keyring's mock backend; the
  encrypted-file path tested directly (round-trip).

No live network calls in CI.

## Open questions

(Things deliberately deferred — call out at implementation time, not now.)

- Should `bgg list` output a default human table or JSON? Default to a compact
  table; `--json` for scripts.
- Do we need `--username` overrides on subcommands, or is "one username per
  install" enough? Start with one; revisit if it bites.
- What subset of collection fields do we persist? Start with the BGG
  `<item>` attributes (objectid, subtype, collid) plus `<name>`, `<yearpublished>`,
  `<image>`, `<thumbnail>`, `<status own/prevowned/.../wishlist/wishlistpriority>`,
  `<numplays>`, and `<stats>` rating/usersrated/average/bayesaverage. Extend as
  needed.

## Out of scope for this scaffold

- Publishing to crates.io, release automation, shell completion generation.
  All easy to add later; not needed to validate the design.
