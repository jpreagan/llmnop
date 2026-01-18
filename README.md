<p align="center">
  <img src="assets/llmnop.png" alt="llmnop" width="420">
</p>

<p align="center">
  <a href="#installation">Installation</a> | <a href="#quick-start">Quick Start</a> | <a href="#what-it-measures">Metrics</a> | <a href="#examples">Examples</a>
</p>

`llmnop` benchmarks LLM inference endpoints. Point it at any OpenAI-compatible API and measure TTFT, inter-token latency, throughput, and end-to-end latency.

## Installation

```bash
# Homebrew
brew install jpreagan/tap/llmnop

# Or with the shell installer
curl -sSfL https://github.com/jpreagan/llmnop/releases/latest/download/llmnop-installer.sh | sh
```

The shell installer places `llmnop` in `~/.local/bin`. Make sure that's on your `PATH`.

## Quick Start

```bash
llmnop --url http://localhost:8000/v1 \
  --api-key token-abc123 \
  --model Qwen/Qwen3-4B-Instruct-2507 \
  --mean-output-tokens 150
```

Results print to stdout and save to `result_outputs/`.

## What It Measures

| Metric              | Description                                                   |
| ------------------- | ------------------------------------------------------------- |
| TTFT                | Time to first token (content or reasoning)                    |
| TTFO                | Time to first output token (content only, excludes reasoning) |
| Inter-token latency | Average time between tokens after the first                   |
| Throughput          | Tokens per second during generation                           |
| End-to-end latency  | Total time from request to completion                         |

TTFO is useful for reasoning models (like DeepSeek-R1) where you want to measure time to actual output, not thinking tokens.

## Configuration

### Required Flags

| Flag        | Description                                 |
| ----------- | ------------------------------------------- |
| `--url`     | Base URL (e.g., `http://localhost:8000/v1`) |
| `--api-key` | API key                                     |

### Options

```
-    --api <API>                      API type [default: chat] [possible values: chat, responses]
    --url <URL>                       Base URL (e.g., http://localhost:8000/v1)
    --api-key <API_KEY>               API key
-m, --model <MODEL>                   Model name (required)
    --tokenizer <TOKENIZER>           Hugging Face tokenizer (defaults to model name)
    --use-server-token-count          Use server-reported token usage for metrics
    --max-num-completed-requests <N>  Number of requests [default: 10]
    --num-concurrent-requests <N>     Parallel requests [default: 1]
    --mean-input-tokens <N>           Target input length [default: 550]
    --stddev-input-tokens <N>         Input length variance [default: 0]
    --mean-output-tokens <N>          Target output length [default: none]
    --stddev-output-tokens <N>        Output length variance [default: 0]
    --results-dir <DIR>               Output directory [default: result_outputs]
    --timeout <SECONDS>               Request timeout [default: 600]
    --no-progress                     Hide progress bar
```

When `--use-server-token-count` is enabled, llmnop uses server-reported token counts for metrics
instead of local tokenization. If the server does not return usage, llmnop will error.

## Examples

Concurrent load testing:

```bash
llmnop --url http://localhost:8000/v1 --api-key token-abc123 \
  --model Qwen/Qwen3-4B-Instruct-2507 \
  --num-concurrent-requests 10 \
  --max-num-completed-requests 100
```

Cap output length for controlled benchmarks:

```bash
llmnop --url http://localhost:8000/v1 --api-key token-abc123 \
  --model Qwen/Qwen3-4B-Instruct-2507 \
  --mean-output-tokens 150
```

Responses API:

```bash
llmnop --api responses --url http://localhost:8000/v1 --api-key token-abc123 \
  --model Qwen/Qwen3-4B-Instruct-2507
```

Custom tokenizer when model name doesn't match Hugging Face:

```bash
llmnop --url http://localhost:8000/v1 --api-key token-abc123 \
  --model gpt-oss:20b \
  --tokenizer openai/gpt-oss-20b
```

Neutral tokenizer for cross-model comparisons:

```bash
llmnop --url http://localhost:8000/v1 --api-key token-abc123 \
  --model gpt-oss:20b \
  --tokenizer hf-internal-testing/llama-tokenizer
```

## Output

Each run produces two JSON files in `result_outputs/`:

| File                                                 | Contents                          |
| ---------------------------------------------------- | --------------------------------- |
| `{model}_{input}_{output}_summary.json`              | Aggregated stats with percentiles |
| `{model}_{input}_{output}_individual_responses.json` | Per-request timing data           |

`{input}` and `{output}` are the mean token counts used for the run.

## License

[Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0)
