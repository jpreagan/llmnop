---
name: release
description: Prepare and publish an llmnop release - release PR, tag, GitHub release, Homebrew formula, and crates.io. Use when the user asks to cut, prepare, or publish a new version.
argument-hint: "[X.Y.Z]"
disable-model-invocation: true
---

# Release llmnop

Release version `$ARGUMENTS`. If no version is given, pick one from the `[Unreleased]` changelog entries by semver (patch for fixes, minor for features, major for breaking changes) and confirm it with the user.

## Prepare

1. Check that the working tree is clean and the tag does not exist locally or on origin: `git tag --list vX.Y.Z`, `git ls-remote --tags origin vX.Y.Z`.
2. Create `release/vX.Y.Z` from `origin/main`. If the release sits on top of open stacked PRs, branch from the top of the stack instead and add the PR to that stack.
3. Set `version` in `Cargo.toml` and run `cargo check` to sync `Cargo.lock`.
4. In `CHANGELOG.md`, add `## [X.Y.Z]` directly below `## [Unreleased]`, leaving `[Unreleased]` empty. Entries are written in each feature PR; check they are user-facing and each links to its PR.
5. Read `README.md` against the release's changes and fix anything out of date.
6. Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`.
7. Run `cargo publish --locked --dry-run` and check the packaged file list for anything that should not ship.
8. Commit as `release X.Y.Z`, push, and open a PR titled `chore(release): X.Y.Z` with the body:

   ```markdown
   ## Summary

   Prepare `X.Y.Z` release
   ```

## Publish

Pushing the tag and publishing the crate cannot be undone. Get the user's go-ahead before step 10 unless they already asked for the release to be published.

9. After the PR merges with green CI: `git checkout main && git pull --ff-only`.
10. `git tag vX.Y.Z && git push origin vX.Y.Z`. This starts the cargo-dist release workflow, which builds every target, creates the GitHub release, and updates the Homebrew formula.
11. Wait for the workflow: `gh run watch <run-id> --exit-status`. If any job fails, stop and report it; do not publish the crate.
12. `cargo publish --locked`. Publish only after the workflow succeeds, because a crates.io version cannot be replaced.

## Verify

- `gh release view vX.Y.Z` lists binaries for every target in `dist-workspace.toml` and the installer.
- The formula in `jpreagan/homebrew-tap` has `version "X.Y.Z"`.
- crates.io reports `X.Y.Z` as the latest version.

Never put secrets or token values in the changelog, logs, or PR descriptions.
