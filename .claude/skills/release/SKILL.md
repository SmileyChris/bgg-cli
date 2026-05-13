---
name: release
description: Cut a new bgg-cli release. Drafts friendly notes from the commits since the last tag, suggests a semver bump, waits for confirmation, then bumps Cargo.toml, commits, tags, pushes, and creates a GitHub release. The release workflow in .github/workflows/release.yml picks up the tag and attaches cross-platform binaries.
---

# Cutting a release

Follow these steps. **Pause for one user confirmation** in step 5 — everything before it is just drafting, nothing is pushed.

## 1. Verify the working tree is clean

```bash
git status --porcelain
```

If anything is uncommitted, stop and tell the user — releases must be cut from a clean tree.

Also confirm we're on `main` (or whichever branch the user has named as the release branch). If not, ask.

## 2. Find the last release tag and read commits since

```bash
git describe --tags --abbrev=0
```

That gives the previous tag (e.g. `v0.1.0`). If no tags exist yet, treat the first commit as the baseline and call this the initial release.

Then read the commits added since:

```bash
git log <prev-tag>..HEAD --reverse --format='%h %s%n%b%n---'
```

Skim the bodies too — sometimes the *why* is in the body, not the subject.

## 3. Draft friendly release notes

**Not a raw git log.** A human-readable summary that someone seeing this release for the first time can read in 30 seconds. Aim for:

- A short opening line setting the theme of the release (e.g. "Filter overhaul + cross-platform binaries.").
- 3–6 bullets grouped by **theme**, not by commit. Multiple commits can collapse into one bullet if they're the same feature.
- Each bullet leads with **what the user can now do** or **what changed for them**, not implementation details.
- Mention breaking changes prominently with a `**Breaking**:` prefix.
- Skip pure-cleanup commits (formatter, comment tweaks) unless they're user-visible.
- No emoji unless the user has used them in prior releases or asks for them.

Markdown is fine — the GitHub release page renders it.

## 4. Suggest the next version

Look at what you wrote in step 3. Pick:

- **major** bump (X.0.0): any `**Breaking**` bullet, or removed/renamed user-facing commands or flags.
- **minor** bump (0.X.0): any new feature, new subcommand, new flag, new output mode.
- **patch** bump (0.0.X): bug fixes, internal refactors, doc-only changes.

Read the current version from `Cargo.toml` (`grep '^version' Cargo.toml`). Compute the suggested next version.

If the project is still pre-1.0, breaking changes can bump the minor instead of the major — that's the conventional reading of `0.x.y`. Use judgement and mention it in the proposal.

## 5. Show the proposal and wait for confirmation

Present, in this order:

```
Proposed release: vX.Y.Z  (was vA.B.C — <reason: feature, fix, breaking>)

<the drafted notes>
```

Then ask: "Tag and push this release?"

**Do not proceed until the user says yes** (or yes-with-edits — apply edits and re-confirm if substantive).

## 6. Apply (only after confirmation)

Run in this order. Stop on any failure. Ordering matters — see "Why this order" below.

```bash
# Bump the version in Cargo.toml. Use the Edit tool over sed when possible
# (avoids the macOS `sed -i ""` quirk).
# Then refresh Cargo.lock and sanity-check:
cargo check
cargo test

# Commit the version bump and push, but DO NOT create the tag locally yet.
git add Cargo.toml Cargo.lock
git commit -m "release: vX.Y.Z"
git push origin HEAD

# Write notes to a temp file (preserves markdown, avoids shell quoting traps).
NOTES=$(mktemp)
cat > "$NOTES" <<'EOF'
<the notes from step 3>
EOF

# Create the GitHub release. With --target, gh creates the tag from the
# given ref on the remote — which immediately fires the release workflow
# AND attaches our notes to the release that the workflow will upload to.
gh release create vX.Y.Z --target main --title "vX.Y.Z" --notes-file "$NOTES"

rm "$NOTES"

# Pull the tag back down locally so the workspace knows about it.
git fetch --tags
```

Don't use `--generate-notes` — we have hand-crafted notes and don't want GitHub clobbering them.

### Why this order

The release workflow uses `taiki-e/upload-rust-binary-action`, which uploads to an existing release when one is present and otherwise creates an empty one. By creating the release **before** the tag is pushed, we guarantee the workflow appends binaries to the release that already has our notes, instead of racing it and ending up with an empty release.

Pushing the commit before `gh release create --target main` is what makes the new tag point at the right SHA on the remote.

## 7. Report back

Print:

- The release URL (from `gh release view vX.Y.Z --json url -q .url`).
- A note that the GitHub Actions workflow is now building cross-platform binaries and will attach them to the release shortly.
- Offer to watch the workflow run with `gh run watch` if the user wants.

## Notes

- Never use `--no-verify` to skip hooks.
- Never force-push a tag.
- If a tag already exists, stop and ask — don't silently overwrite.
- The release workflow is at `.github/workflows/release.yml` and triggers on tag push matching `v*`.
