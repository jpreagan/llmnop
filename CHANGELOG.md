# Changelog

## Unreleased

### Performance

- Reduce startup latency by parallelizing corpus tokenization.

### Enhancements

- Add Responses API support with `--api` plus CLI-configured `--url` and `--api-key`. ([#27](https://github.com/jpreagan/llmnop/pull/27))
- Add `--use-server-token-count` to use API-reported usage for token metrics. ([#30](https://github.com/jpreagan/llmnop/pull/30))

## 0.5.0

Released on 2026-01-13

### Performance

- Faster prompt generation via tokenize-once caching. ([#23](https://github.com/jpreagan/llmnop/pull/23))

### Enhancements

- Precise token targeting (token-level sampling replaces line-level). ([#23](https://github.com/jpreagan/llmnop/pull/23))
- Per-request `max_tokens` sampling when `--mean-output-tokens` is specified. ([#23](https://github.com/jpreagan/llmnop/pull/23))
- Expanded Shakespeare corpus supports larger input token requests. ([#23](https://github.com/jpreagan/llmnop/pull/23))
- Updated defaults: `--stddev-input-tokens` and `--stddev-output-tokens` now 0, `--max-num-completed-requests` now 10. ([#23](https://github.com/jpreagan/llmnop/pull/23))

## 0.4.0

Released on 2026-01-04

### Enhancements

- Support reasoning models with correct token counts, throughput, and latency metrics. ([#16](https://github.com/jpreagan/llmnop/pull/16))
- Add `--mean-output-tokens` as optional (default: none) to avoid constraining model output. ([#16](https://github.com/jpreagan/llmnop/pull/16))

### Bug fixes

- Compute per-request output throughput over the generation window (first streamed token → last streamed token), rather than full request wall time.

## 0.3.1

Released on 2025-10-31

### Enhancements

- `--no-progress` CLI flag to disable the progress bar for non-interactive environments.

## 0.3.0

Released on 2025-10-27.

### Enhancements

- Allow different tokenizer than served model via `--tokenizer`; write `tokenizer` to summary JSON and bump schema to `2025-10-05`. ([#1](https://github.com/jpreagan/llmnop/pull/1))
- Add shell installer + Homebrew formula; default install now respects `$XDG_BIN_HOME` (or `~/.local/bin`). ([#2](https://github.com/jpreagan/llmnop/pull/2))
- Increase default `--max-num-completed-requests` to 2. ([#3](https://github.com/jpreagan/llmnop/pull/3))
- Build `tokenizers` with `rustls-tls` (drops OpenSSL dependency). ([#4](https://github.com/jpreagan/llmnop/pull/4))

## 0.2.0

Released on 2025-07-12.

### Enhancements

- Upgrade to Rust 2024 edition (MSRV v1.85).
- Add benchmark summary output.
- Flatten benchmark-results JSON.

## 0.1.0

Released on 2025-07-09.

Initial release.
