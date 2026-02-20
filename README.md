<p align="center">
  <img src="assets/llmnop.png" alt="llmnop" width="420">
</p>

<p align="center">
  <a href="#installation">Installation</a> | <a href="#quick-start">Quick Start</a> | <a href="#what-it-measures">Metrics</a> | <a href="#examples">Examples</a>
</p>

`llmnop` is a fast, lightweight CLI that benchmarks LLM inference endpoints with detailed latency and throughput metrics.

It's a single binary with no dependencies, just download and run. Use it to compare inference providers, validate deployment performance, tune serving parameters, or establish baselines before and after changes.

## Installation

Use the installer:

```bash
curl -sSfL https://github.com/jpreagan/llmnop/releases/latest/download/llmnop-installer.sh | sh
```

It places `llmnop` in `~/.local/bin`. Make sure that's on your `PATH`.

Or use Homebrew:

```bash
brew install jpreagan/tap/llmnop
```

## Updating

If you used the installer, update in place:

```bash
llmnop update
```

If you used Homebrew:

```bash
brew upgrade llmnop
```

## Quick Start

```bash
llmnop --url http://localhost:8000/v1 \
  --api-key token-abc123 \
  --model Qwen/Qwen3-4B-Instruct-2507 \
  --mean-output-tokens 150
```

Results print to stdout and save under the llmnop app results directory:

- macOS: `~/Library/Application Support/llmnop/results`
- Linux: `${XDG_STATE_HOME:-~/.local/state}/llmnop/results`
- Windows: `%LOCALAPPDATA%\\llmnop\\data\\results`

## What It Measures

| Metric                  | Description                                                     |
| ----------------------- | --------------------------------------------------------------- |
| **TTFT**                | Time to first token - how long until streaming begins           |
| **TTFO**                | Time to first output token - excludes reasoning/thinking tokens |
| **Inter-token latency** | Estimated average time between generated tokens                 |
| **Inter-event latency** | Average gap between streamed events/chunks                      |
| **Throughput**          | Tokens per second during the generation window                  |
| **End-to-end latency**  | Total request time from start to finish                         |

### Notes

- For reasoning models, TTFT includes thinking tokens.
- TTFO measures time until actual output begins, so it better reflects user-perceived latency.
- Inter-event latency captures stream chunk cadence.
- Inter-token latency is token-count based and less sensitive to chunk batching.

## Configuration

### Endpoint

| Flag            | Description                                 |
| --------------- | ------------------------------------------- |
| `--url`         | Base URL (e.g., `http://localhost:8000/v1`) |
| `--api-key`     | API key for authentication                  |
| `--model`, `-m` | Model name to benchmark                     |
| `--api`         | API type: `chat` (default) or `responses`   |

`chat` targets OpenAI's [Chat Completions API](https://platform.openai.com/docs/api-reference/chat). `responses` targets the [Responses API](https://platform.openai.com/docs/api-reference/responses) format, compatible with both OpenAI and [Open Responses](https://huggingface.co/blog/open-responses) servers.

### Request Shaping

Control input and output token counts to simulate realistic workloads:

| Flag                     | Default | Description                                               |
| ------------------------ | ------- | --------------------------------------------------------- |
| `--mean-input-tokens`    | 550     | Target prompt length in tokens                            |
| `--stddev-input-tokens`  | 0       | Add variance to input length                              |
| `--mean-output-tokens`   | none    | Cap output length (recommended for consistent benchmarks) |
| `--stddev-output-tokens` | 0       | Add variance to output length                             |

### Load Testing

| Flag                           | Default | Description                |
| ------------------------------ | ------- | -------------------------- |
| `--max-num-completed-requests` | 10      | Total requests to complete |
| `--num-concurrent-requests`    | 1       | Parallel request count     |
| `--timeout`                    | 600     | Request timeout in seconds |

### Tokenization

By default, llmnop uses a local Hugging Face tokenizer matching `--model` to count tokens.

| Flag                       | Description                                                               |
| -------------------------- | ------------------------------------------------------------------------- |
| `--tokenizer`              | Use a different HF tokenizer (when model name doesn't match Hugging Face) |
| `--use-server-token-count` | Use server-reported usage instead of local tokenization                   |

Use `--use-server-token-count` when you trust the server's token counts and want to avoid downloading tokenizer files. The server must return usage data or llmnop will error.

### Output

| Flag              | Default | Description                                      |
| ----------------- | ------- | ------------------------------------------------ |
| `--json`          | false   | Emit benchmark summary JSON to stdout            |
| `--output-format` | `table` | Stdout output format: `table`, `json`, or `none` |
| `--quiet`, `-q`   | false   | Suppress stdout output (`--output-format none`)  |

## Examples

**Load test with concurrency:**

```bash
llmnop --url http://localhost:8000/v1 --api-key token-abc123 \
  --model Qwen/Qwen3-4B-Instruct-2507 \
  --num-concurrent-requests 10 \
  --max-num-completed-requests 100
```

**Controlled benchmark with fixed output length:**

```bash
llmnop --url http://localhost:8000/v1 --api-key token-abc123 \
  --model Qwen/Qwen3-4B-Instruct-2507 \
  --mean-output-tokens 150
```

**Responses API:**

```bash
llmnop --api responses --url http://localhost:8000/v1 --api-key token-abc123 \
  --model openai/gpt-oss-120b
```

**JSON stdout for `jq` pipelines:**

```bash
llmnop --url http://localhost:8000/v1 --api-key token-abc123 \
  --model Qwen/Qwen3-4B-Instruct-2507 \
  --output-format json \
  --max-num-completed-requests 1 | jq '.request_latency.p99'
```

**Custom tokenizer when model name doesn't match Hugging Face:**

```bash
llmnop --url http://localhost:11434/v1 --api-key ollama
  --model gpt-oss:20b \
  --tokenizer openai/gpt-oss-20b
```

**Cross-model comparison with neutral tokenizer:**

When comparing different models, use a consistent tokenizer so token counts are comparable:

```bash
llmnop --url http://localhost:8000/v1 --api-key token-abc123 \
  --model Qwen/Qwen3-4B-Instruct-2507 \
  --tokenizer hf-internal-testing/llama-tokenizer
```

## Output Files

Each run writes artifacts to a per-run directory:

- macOS: `~/Library/Application Support/llmnop/results`
- Linux: `${XDG_STATE_HOME:-~/.local/state}/llmnop/results`
- Windows: `%LOCALAPPDATA%\\llmnop\\data\\results`

Path layout:

- `<results>/<benchmark_slug>/<run_id>/summary.json`
- `<results>/<benchmark_slug>/<run_id>/individual_responses.jsonl`

| File                         | Contents                                                                 |
| ---------------------------- | ------------------------------------------------------------------------ |
| `summary.json`               | Aggregated benchmark metrics using nested metric objects (`unit`, stats) |
| `individual_responses.jsonl` | Per-request records with `metadata`, `metrics`, and `error` (JSONL)      |

The summary includes statistical breakdowns for latency and token metrics. `individual_responses.jsonl` stores one request record per line for efficient processing on larger runs.

## License

[Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0)
