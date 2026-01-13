---
name: release
description: Create a release for llmnop. Use when making a new version release, bumping version, or preparing a release PR.
allowed-tools: Read, Edit, Write, Bash, Glob, Grep
---

# Release Process

## Steps

1. **Create release branch**: `git checkout -b release/vX.Y.Z`

2. **Bump version** in `Cargo.toml`, run `cargo check` (syncs `Cargo.lock`)

3. **Update CHANGELOG.md**: Rename `## Unreleased` to `## X.Y.Z` with release date. Verify all entries have correct PR links.

4. **Dry-run publish**: run `cargo publish --locked --dry-run` and check output for extraneous files

5. **Commit**: `git commit -m "release vX.Y.Z"`

6. **Push and create PR**: Target `main`, title `chore(release): X.Y.Z`

7. **After PR merge** (user confirms):

   ```bash
   git checkout main && git pull --ff-only
   git tag vX.Y.Z && git push origin vX.Y.Z
   ```

8. **Publish**: `cargo publish --locked`

## Conventions

- Follow semver: patch for fixes, minor for features, major for breaking changes
- Branch: `release/vX.Y.Z`
- PR title: `chore(release): X.Y.Z`
