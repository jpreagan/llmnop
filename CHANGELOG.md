# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project adheres to Semantic Versioning.

## [Unreleased]

### Added

- Support local tokenizer JSON files through `--tokenizer`. ([#69](https://github.com/jpreagan/llmnop/pull/69))
- Add `--warmup` for requests completed before measurement and excluded from benchmark statistics. ([#71](https://github.com/jpreagan/llmnop/pull/71))
- Restore `--results-dir`. ([#72](https://github.com/jpreagan/llmnop/pull/72))

### Changed

- Rename `--mean-input-tokens` to `--input-tokens`, `--stddev-input-tokens` to `--input-tokens-stddev`, `--mean-output-tokens` to `--output-cap`, `--stddev-output-tokens` to `--output-cap-stddev`, and `--thinking-budget-tokens` to `--thinking-budget`. ([#69](https://github.com/jpreagan/llmnop/pull/69))
- Replace `--max-num-completed-requests` and `--num-concurrent-requests` with `--requests` and `--concurrency`. Failed attempts count toward the request budget and are not retried. ([#71](https://github.com/jpreagan/llmnop/pull/71))
- Replace the benchmark-wide `--timeout` with a per-request `--request-timeout`. Save partial results on timeout or interruption. ([#71](https://github.com/jpreagan/llmnop/pull/71))
- Rename `individual_responses.jsonl` to `requests.jsonl` and revise the JSON result schema to include separate warmup outcomes, sample counts, and null values for unavailable measurements. Calculate summary statistics from completed measurement requests. ([#72](https://github.com/jpreagan/llmnop/pull/72))
- Replace `--output-format`, `--json`, and `--quiet` with `--format table|json|none`, and rename `--use-server-token-count` to `--request-usage`. ([#72](https://github.com/jpreagan/llmnop/pull/72))

### Removed

- Remove the redundant `version` field from summary JSON. Use `schema_version` to identify the output format and `llmnop_version` to identify the producing release. ([#72](https://github.com/jpreagan/llmnop/pull/72))

### Fixed

- Ensure generated prompts match their requested token counts or fail explicitly when that target cannot be met. ([#69](https://github.com/jpreagan/llmnop/pull/69))
- Report malformed or prematurely terminated streams as failed requests, and calculate content and reasoning timings consistently across supported APIs, leaving measurements absent when no qualifying text arrives. ([#70](https://github.com/jpreagan/llmnop/pull/70))

## [0.10.0]

### Added

- Add Anthropic Messages API support via `--api messages`. ([#36](https://github.com/jpreagan/llmnop/pull/36))
- Add `--thinking-budget-tokens` for Anthropic Messages API extended thinking. ([#36](https://github.com/jpreagan/llmnop/pull/36))
- Add separate provider usage token metrics in JSON output while preserving measured output/reasoning token metrics. ([#64](https://github.com/jpreagan/llmnop/pull/64))

### Changed

- `--use-server-token-count` now requests provider usage reporting instead of replacing llmnop's measured token metrics. ([#64](https://github.com/jpreagan/llmnop/pull/64))

### Security

- Prevent ambient `OPENAI_API_KEY` from being sent to benchmark endpoints when `--api-key` is omitted. ([#61](https://github.com/jpreagan/llmnop/pull/61))

## [0.9.0]

### Changed

- Benchmark JSON output now always writes to the platform app data/state directory for llmnop instead of the current working directory. ([#51](https://github.com/jpreagan/llmnop/pull/51))
- Benchmark results are now written to per-run subdirectories to avoid overwriting repeated runs with the same configuration. ([#52](https://github.com/jpreagan/llmnop/pull/52))
- Summary JSON now uses nested metric objects, and run artifacts are written as `summary.json` and `individual_responses.jsonl`. ([#53](https://github.com/jpreagan/llmnop/pull/53))
- Added `--output-format table|json|none` and `--json` for stdout rendering, including machine-readable JSON output for `jq` pipelines. ([#57](https://github.com/jpreagan/llmnop/pull/57))

### Fixed

- Responses API benchmarking now handles Ollama `response.reasoning_summary_text.delta` stream events so TTFT/inter-event and reasoning token metrics are populated correctly. ([#54](https://github.com/jpreagan/llmnop/pull/54))

### Removed

- Removed `--results-dir`; output location is now managed automatically. ([#51](https://github.com/jpreagan/llmnop/pull/51))

## [0.8.0]

### Added

- `inter_event_latency` metric for streamed event/chunk cadence. ([#47](https://github.com/jpreagan/llmnop/pull/47))

### Changed

- `inter_token_latency` is now token-count-based (robust to batched stream events). ([#47](https://github.com/jpreagan/llmnop/pull/47))
- Clarified inter-token vs inter-event metric semantics in the README. ([#47](https://github.com/jpreagan/llmnop/pull/47))

## [0.7.1]

### Added

- `llmnop update` command for standalone installs. ([#44](https://github.com/jpreagan/llmnop/pull/44))

## [0.7.0]

### Added

- `-q`/`--quiet` flag to suppress stdout output. ([#35](https://github.com/jpreagan/llmnop/pull/35))
- Optional `--api-key` support for unauthenticated servers. ([#38](https://github.com/jpreagan/llmnop/pull/38))

### Removed

- `--no-progress`; use `--quiet` to suppress progress output. ([#37](https://github.com/jpreagan/llmnop/pull/37))

## [0.6.0]

### Added

- Responses API support via `--api`, with CLI-configured `--url` and `--api-key`. ([#27](https://github.com/jpreagan/llmnop/pull/27))
- `--use-server-token-count` to use API-reported usage for token metrics. ([#30](https://github.com/jpreagan/llmnop/pull/30))

### Changed

- Improved startup latency by parallelizing corpus tokenization. ([#32](https://github.com/jpreagan/llmnop/pull/32))

## [0.5.0]

### Added

- Precise token targeting (token-level sampling replaces line-level). ([#23](https://github.com/jpreagan/llmnop/pull/23))
- Per-request `max_tokens` sampling when `--mean-output-tokens` is specified. ([#23](https://github.com/jpreagan/llmnop/pull/23))
- Expanded Shakespeare corpus to support larger input token requests. ([#23](https://github.com/jpreagan/llmnop/pull/23))

### Changed

- Faster prompt generation via tokenize-once caching. ([#23](https://github.com/jpreagan/llmnop/pull/23))
- Updated defaults: `--stddev-input-tokens=0`, `--stddev-output-tokens=0`, `--max-num-completed-requests=10`. ([#23](https://github.com/jpreagan/llmnop/pull/23))

## [0.4.0]

### Added

- Reasoning-model support with correct token counts, throughput, and latency metrics. ([#16](https://github.com/jpreagan/llmnop/pull/16))
- `--mean-output-tokens` can be omitted (default: none) to avoid constraining output length. ([#16](https://github.com/jpreagan/llmnop/pull/16))

### Fixed

- Per-request output throughput now uses generation window (first streamed token to last streamed token), not full request wall time. ([#15](https://github.com/jpreagan/llmnop/pull/15))

## [0.3.1]

### Added

- `--no-progress` flag for non-interactive environments. ([#9](https://github.com/jpreagan/llmnop/pull/9))

## [0.3.0]

### Added

- `--tokenizer` to allow a tokenizer different from served model; `tokenizer` field in summary JSON and schema bump to `2025-10-05`. ([#1](https://github.com/jpreagan/llmnop/pull/1))
- Installer script and Homebrew formula; default install respects `$XDG_BIN_HOME` (or `~/.local/bin`). ([#2](https://github.com/jpreagan/llmnop/pull/2))

### Changed

- Default `--max-num-completed-requests` increased to `2`. ([#3](https://github.com/jpreagan/llmnop/pull/3))
- `tokenizers` built with `rustls-tls` (drops OpenSSL dependency). ([#4](https://github.com/jpreagan/llmnop/pull/4))

## [0.2.0]

### Added

- Benchmark summary output.
- Flattened benchmark-results JSON.

### Changed

- Upgraded to Rust 2024 edition (MSRV v1.85).

## [0.1.0]

### Added

- Initial release.
