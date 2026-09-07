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
llmnop --url http://localhost:11434/v1 \
  --api responses \
  --model qwen3.8:27b-mlx \
  --tokenizer Qwen/Qwen3.8-27B \
  --input-tokens 550 --output-cap 128 \
  --requests 20 --concurrency 4
```

`--url` and `--model` are required for benchmarks. Authentication is optional.
Use `llmnop --help` for the complete option list. Commands such as `update`
work without an endpoint; self-update is available in standalone builds.

## What It Measures

At a specified concurrency, llmnop measures how quickly each request receives
output and completes, and how much completed work the endpoint delivers.
Each finished attempt frees a slot for another request. Failures count toward
`--requests`; there are no automatic retries or redirects. Concurrency is a
ceiling, with fewer active requests during startup and the final drain.

All API modes stream: `chat` uses `/chat/completions`, `responses` uses
`/responses`, and `messages` uses `/messages`. Paths are appended to the supplied
base URL, including any version prefix. The base URL must not contain credentials,
a query, or a fragment. `--api-key` supplies bearer authentication for Chat and
Responses, or `x-api-key` authentication for Messages.

| Measurement | Definition |
| --- | --- |
| TTFT | Request start to the first nonempty content or exposed reasoning delta. |
| TTFO | Request start to the first nonempty content delta. |
| Request latency | Request start to the API's terminal completion event. |
| Generation window | First to last nonempty content or reasoning event. |
| Generation rate | `(generated_tokens - 1) / generation_window_seconds`. |
| Estimated inter-token latency | `generation_window_ms / (generated_tokens - 1)`. |
| Mean stream-event gap | Generation window divided by the number of qualifying events minus one. |
| Longest stream-event gap | Maximum interval between qualifying events. |
| Input tokens | Locally tokenized prompt text, excluding server-added formatting. |
| Content tokens | Locally tokenized streamed response content, including streamed refusals. |
| Exposed reasoning tokens | Locally tokenized streamed reasoning text or summaries. |
| Generated tokens | Content tokens plus exposed reasoning tokens. |

Timing starts immediately before sending a prepared HTTP request and includes
connection setup, endpoint queueing, and network delays. It excludes local prompt
preparation and waiting for a concurrency slot. A valid terminal event is `[DONE]`
for Chat, a completed response or supported normal limit/filter termination for
Responses, and `message_stop` for Messages. EOF alone does not establish success.
A Chat finish reason is retained while waiting for `[DONE]` and any intervening usage.

TTFT and TTFO are client observations of text-bearing events, not individual model
tokens. A single event can carry multiple tokens or both content and reasoning;
it counts as one event. Metadata, heartbeats, and tool-call arguments do not count
as text delivery. Inter-token latency and generation rate are token-normalized
estimates, not measurements of individual model token intervals. Neither measures
pure prefill speed or internal reasoning duration.

Unobservable measurements are JSON `null` and shown as `—` in the table. No content
means no TTFO. No text events means no TTFT. A single event has a zero generation
window but no measurable generation rate or event gap. Token-normalized rate and
latency require at least two locally measured tokens and a positive window.
Zero observed reasoning tokens does not establish that the model did no reasoning.
Reasoning representations are recorded in `reasoning_kinds` (`text`, `summary`).

## Workload

The sole workload is sampled Shakespeare text. The corpus matches the sonnet corpus
in [AIPerf](https://github.com/ai-dynamo/aiperf). Nonempty stripped lines are joined
with spaces in fixed 10,000-character chunks, then tokenized once. Each prompt
samples a random contiguous token window, wrapping around the corpus as needed.

`--input-tokens` sets the mean token target, and `--input-tokens-stddev` sets its
standard deviation. With a zero standard deviation the target is fixed. Otherwise,
lengths are sampled from a normal distribution conditioned on nonnegative values,
rounded upward, with a minimum of one. Decoded prompts are re-encoded and trimmed
or topped up to meet the target. Preparation fails after ten unsuccessful
adjustments rather than silently accepting a different size.

These mechanics follow AIPerf's sonnet generator, with independent checks for
round-trip length and arbitrary corpus wrapping.

`--output-cap` and `--output-cap-stddev` control the requested generation cap,
not the actual response length. Responses may stop earlier or an endpoint may
ignore an unsupported parameter. API mappings are:

| API | Cap field |
| --- | --- |
| Chat | `max_completion_tokens` |
| Responses | `max_output_tokens` |
| Messages | `max_tokens` (required) |

Some Ollama Chat implementations ignore `max_completion_tokens`. In that case,
use Responses or Messages for controlled caps. llmnop records both requested caps
and actual locally measured counts; it does not infer a limit finish merely from
those counts. Reasoning's contribution to the provider's cap depends on its API
and model.

`--thinking-budget` enables Messages thinking. It must be at least 1024 and below
the mean output cap. Every sampled cap also exceeds the thinking budget.
A nonzero output-cap standard deviation requires an output cap.

`--tokenizer` accepts a Hugging Face identifier or a local `tokenizer.json` file.
The default identifier is the model name. Tokenizer truncation and padding are
disabled for measurement. Tokenization of assembled content and reasoning occurs
after streaming, outside request timing, on blocking workers. A slot is replenished
without waiting for that tokenization. Text is discarded after counting.

## Run Controls

`--requests` defaults to 10 measured attempts. `--concurrency` defaults to 1.
`--warmup` defaults to 0 and specifies a total number of additional attempts, using
the same concurrency ceiling. Warmup finishes before measured requests begin; its
records and outcome counts are retained separately. Warmup does not guarantee any
particular server cache state.

`--request-timeout` is a per-request deadline in seconds, including the complete
stream. Fractional seconds are supported. A timed-out request is cancelled locally
and retains observations received before its deadline. Ctrl-C stops dispatching
requests, cancels active requests, and writes the available results. The endpoint
may take additional time to stop its own work after a client disconnects.

## Results

Every run writes a unique directory containing:

- `summary.json`: configuration, outcomes, totals, and statistics.
- `requests.jsonl`: one measurement record per attempted request, including warmup.

`summary.json` records `schema_version` for the structure and semantics of both result files, and `llmnop_version` for the release that produced them.

Use `--results-dir` to choose the parent directory. Defaults are:

- macOS: `~/Library/Application Support/llmnop/results`
- Linux: `${XDG_STATE_HOME:-$HOME/.local/state}/llmnop/results`
- Windows: the platform-local llmnop data directory, under `results`

`--format table|json|none` controls stdout only. Progress, errors, and the result location go to stderr. Progress is shown only when stderr is a terminal. Prompt text, response text, credentials, and individual event histories are not exported.

Each request record contains its stable ID and phase, timestamps, elapsed time,
status, HTTP status, provider finish reason, sampled input target, requested output
cap, metrics, reasoning representation, provider usage, and error details.
Records are appended as accounting completes and need not be in request-ID order.
Failed and cancelled requests retain partial observations; `request_latency_ms`
is null for these requests, while `elapsed_ms` records time to their terminal outcome.

Durations are measured using a monotonic clock. Exported Unix nanosecond timestamps
share a fixed wall-clock anchor so changes to the system clock during a run cannot
change durations or cross-request timing. The aggregate measurement interval runs
from the first measured request's start to the last measured request's termination,
including failures. Preparation, warmup, and final reporting are excluded.

Summary metric distributions include **completed measured requests only**. Every
metric has its own sample count, excluding null observations. Statistics are the
arithmetic mean, population standard deviation, min/max, and linearly interpolated
percentiles at indices `p * (n - 1)`. A small sample's p99 is descriptive and does
not establish a reliable population tail estimate.

Aggregate request throughput is completed measured requests divided by the
measurement interval. Aggregate token throughput is locally counted generated
tokens from those completed requests divided by the same interval. Partial tokens
from unsuccessful requests remain in the request records and do not enter
completed-work throughput. Output-limit completions are counted separately from
failures; empty text completions can still be successful protocol completions.

`--request-usage` asks Chat endpoints to include optional streaming usage. Usage
returned by any API is always saved separately in `provider_usage`, using that
API's field names. For Messages, later non-null usage fields update earlier ones. Provider
counts never replace local counts or enter local rate calculations; reasoning
subcounts must not be added again to provider totals that already include them.

Exit status is 0 when all attempted requests complete, 1 for request/setup/export
failures, 2 for invalid arguments, and 130 for an interrupted run. Results are
preserved for normal request failures, deadlines, and interruptions.

### Schema 3 migration

This overhaul changes the JSON schema to `3.0`. Scalar run values are plain numbers;
per-request metrics use unit-bearing names and unavailable values are null. Every
summary distribution includes a sample count. The old `individual_responses.jsonl`
becomes `requests.jsonl`.

| Previous option | Replacement |
| --- | --- |
| `--mean-input-tokens` | `--input-tokens` |
| `--stddev-input-tokens` | `--input-tokens-stddev` |
| `--mean-output-tokens` | `--output-cap` |
| `--stddev-output-tokens` | `--output-cap-stddev` |
| `--thinking-budget-tokens` | `--thinking-budget` |
| `--max-num-completed-requests` | `--requests`, `-n` |
| `--num-concurrent-requests` | `--concurrency`, `-c` |
| `--timeout` | `--request-timeout` (now a per-request deadline) |
| `--use-server-token-count` | `--request-usage` |
| `--output-format`, `--json`, `--quiet` | `--format table|json|none` |

Per-request generation rate now uses `(generated_tokens - 1) / window`, making it
the reciprocal of estimated inter-token latency. Previous releases used
`generated_tokens / window`. Aggregate throughput still counts all generated tokens.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Tests include local HTTP fixtures for all three streaming APIs, fragmented SSE,
partial failures, deadlines, cancellation, concurrency, and exported statistics.
Changes to streaming or measurements should also be exercised against a real
endpoint using the applicable API modes.
