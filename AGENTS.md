# llmnop

CLI tool for benchmarking LLM inference endpoints (OpenAI API compatible). Measures TTFT, inter-token latency, throughput, and end-to-end latency.

## Required CLI Flags

- `--model` - Required model name
- `--url` - Required base URL (e.g., `http://localhost:8000/v1`)
- `--api-key` - Required API key

## Key Metrics

- **TTFT** - Time to first token (content or reasoning)
- **TTFO** - Time to first output token (content only, excludes reasoning)
- **Inter-token latency** - Average gap between token arrivals (excludes TTFT)
- **Throughput** - Tokens/second over generation window (first to last token)

## Code Style

- UNIX philosophy: small, focused, correct code
- Idiomatic Rust (model after tokio, ripgrep, cargo)
- Comments explain why, not what; code should be self-documenting
- Be conservative with dependencies; vet before adding
- Tests verify behavior, not implementation
- Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` before committing

## Git Conventions

- **PR titles**: Conventional Commits `type(scope): description` (lowercase)
  - Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`
- **Commits**: Plain lowercase, no conventional format
- **PRs**: Atomic—one concern per PR

## Changelog

User-facing changes only. Categories: Enhancements, Bug fixes, Documentation, Configuration, Performance.
