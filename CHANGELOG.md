# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2025-10-27

### Added

- `--tokenizer` CLI flag to allow a different tokenizer name than the served model name (e.g., `--model gpt-oss:20b --tokenizer openai/gpt-oss-20b`).  
  Also writes `tokenizer` into the summary JSON and bumps the summary schema version field to `2025-10-05`. ([#1](https://github.com/jpreagan/llmnop/pull/1))

### Changed

- Default `--max-num-completed-requests` from 1 → 2, so two completions can be handled out of the box. ([#3](https://github.com/jpreagan/llmnop/pull/3))
- Dependency refresh; `tokenizers` now built with `rustls-tls` (no OpenSSL dependency). ([#4](https://github.com/jpreagan/llmnop/pull/4))

### CI / Build

- Upgrade `cargo-dist` to v0.30; enable shell + Homebrew installers; use `$XDG_BIN_HOME` / `~/.local/bin` install paths; docs refreshed. ([#2](https://github.com/jpreagan/llmnop/pull/2))

### Notes

- No breaking changes expected. New CLI flag is optional.

## [0.2.0] - 2025-07-12

- Upgraded the project to Rust 2024 edition, and MSRV is now v1.85.
- Added a benchmark summary that prints aggregated statistics to stdout.
- Flattened the benchmark-results JSON structure.

## [0.1.0] - 2025-07-09

Initial release.
