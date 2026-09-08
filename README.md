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
curl -sSfL https://llmnop.xyz/install.sh | sh
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
  --output-cap 150
```

Results print to stdout and save under the llmnop app results directory:

- macOS: `~/Library/Application Support/llmnop/results`
- Linux: `${XDG_STATE_HOME:-$HOME/.local/state}/llmnop/results`
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

| Flag            | Description                                            |
| --------------- | ------------------------------------------------------ |
| `--url`         | Base URL (e.g., `http://localhost:8000/v1`)            |
| `--api-key`     | API key for authentication                             |
| `--model`, `-m` | Model name to benchmark                                |
| `--api`         | API type: `chat` (default), `responses`, or `messages` |

`chat` targets OpenAI's [Chat Completions API](https://platform.openai.com/docs/api-reference/chat). `responses` targets the [Responses API](https://platform.openai.com/docs/api-reference/responses) format, compatible with both OpenAI and [Open Responses](https://huggingface.co/blog/open-responses) servers. `messages` targets Anthropic's [Messages API](https://docs.anthropic.com/en/api/messages).

### Request Shaping

Control input and output token counts to simulate realistic workloads:

| Flag                       | Default | Description                                               |
| -------------------------- | ------- | --------------------------------------------------------- |
| `--input-tokens`      | 550     | Target prompt length in tokens                            |
| `--input-tokens-stddev`    | 0       | Add variance to input length                              |
| `--output-cap`     | none    | Mean output token cap to request                          |
| `--output-cap-stddev`   | 0       | Add variance to output length                             |
| `--thinking-budget` | none    | Enable Anthropic Messages thinking with this token budget |

For `--api messages`, `--output-cap` is required so llmnop can set `max_tokens` in the request.
When `--thinking-budget` is set, it must be at least 1024 and smaller than `--output-cap`.

### Load Testing

| Flag                           | Default | Description                |
| ------------------------------ | ------- | -------------------------- |
| `--max-num-completed-requests` | 10      | Total requests to complete |
| `--num-concurrent-requests`    | 1       | Parallel request count     |
| `--timeout`                    | 600     | Request timeout in seconds |

### Tokenization

By default, llmnop uses a local Hugging Face tokenizer matching `--model` to count measured token metrics.

| Flag                       | Description                                                               |
| -------------------------- | ------------------------------------------------------------------------- |
| `--tokenizer`              | Use a different HF tokenizer (when model name doesn't match Hugging Face) |
| `--use-server-token-count` | Request provider-reported usage and record it separately                 |

Primary token metrics preserve llmnop's measured semantics: `output_token_count` is visible answer tokens, `reasoning_token_count` is reasoning/thinking tokens, and `output_sequence_length` is their sum. Provider usage fields such as `usage_input_tokens`, `usage_output_tokens`, `usage_total_tokens`, and `usage_reasoning_tokens` are recorded separately when available because provider `usage` may be aggregate or unsplit.

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
  --output-cap 150
```

**Responses API:**

```bash
llmnop --api responses --url http://localhost:8000/v1 --api-key token-abc123 \
  --model openai/gpt-oss-120b
```

**Anthropic Messages API:**

```bash
llmnop --api messages --url http://localhost:8000/v1 --api-key token-abc123 \
  --model Qwen/Qwen3.6-27B \
  --output-cap 4096
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
- Linux: `${XDG_STATE_HOME:-$HOME/.local/state}/llmnop/results`
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

Sonnet prompts use a fresh random window from the tokenized Shakespeare corpus.
Prompt text must re-tokenize to its sampled target; generation fails if repair cannot achieve that length.
The tokenizer may be a Hugging Face identifier or a local tokenizer JSON file.

Streaming clients require an explicit completion event; malformed and truncated streams fail.
TTFT includes exposed reasoning; TTFO requires visible text. Missing timings are absent,
and generation metrics exclude both the initial wait and completion metadata after the final text event.
Token counts use assembled text, with provider usage kept separate. HTTP retries and redirects are disabled.
