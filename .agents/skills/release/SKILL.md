---
name: release
description: Create a release for llmnop. Use when preparing a new version, release PR, tag, and crates.io publish.
metadata:
  short-description: Prepare and publish an llmnop release
---

# Release Process

## Preconditions

- Repo is clean.
- Release tag does not already exist (`git tag --list vX.Y.Z`).

## Steps

1. **Create release branch**: `git checkout -b release/vX.Y.Z`
2. **Bump version** in `Cargo.toml`, then run `cargo check` (syncs `Cargo.lock` if needed).
3. **Update changelog** in `CHANGELOG.md`:
   - Keep `## [Unreleased]` at the top.
   - Add `## [X.Y.Z]`.
   - Include user-facing changes only.
   - Ensure each entry links to its corresponding GitHub PR.
4. **Run preflight checks**:
   - `cargo fmt --check`
   - `cargo check`
   - `cargo test`
5. **Dry-run publish**:
   - `cargo publish --locked --dry-run`
   - Review output for extraneous files.
6. **Commit and open PR**:
   - Commit release-prep changes.
   - Push branch and open PR to `main`.
   - PR title: `chore(release): X.Y.Z`
7. **Merge PR** after CI is green.
8. **Sync and tag**:
   - `git checkout main && git pull --ff-only`
   - `git tag vX.Y.Z && git push origin vX.Y.Z`
9. **Publish crate**:
   - `cargo publish --locked`
10. **Verify publish**:

- Confirm release workflow completed successfully.
- Confirm GitHub release exists for `vX.Y.Z`.
- Confirm the crate version is visible on crates.io.

## Conventions

- Follow semver:
  - patch = fixes
  - minor = features
  - major = breaking changes
- Branch: `release/vX.Y.Z`
- PR title: `chore(release): X.Y.Z`
- Tag: `vX.Y.Z`

## Notes

- Do not include secrets or token values in changelog, logs, or PR descriptions.
