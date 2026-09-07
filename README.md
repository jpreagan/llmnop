<p align="center">
  <img src="assets/llmnop.png" alt="llmnop" width="420">
</p>

`llmnop` benchmarks the performance of LLM inference endpoints. It answers the question:

> At concurrency N, how fast is each request on this endpoint under a specified workload?

Use it to compare models, providers, and serving configurations, or see how performance changes as more requests share an endpoint. It measures the experience through the API, including network delays and server queueing.

## Install

With Homebrew:

```bash
brew install jpreagan/tap/llmnop
```

Or with the standalone installer:

```bash
curl -sSfL https://llmnop.xyz/install.sh | sh
```

The installer uses `$XDG_BIN_HOME` if set, otherwise `~/.local/bin`. Make sure the directory is on your `PATH`.

Update with `brew upgrade llmnop` or, for standalone installs, `llmnop update`.

## Run a benchmark

With vLLM serving `Qwen/Qwen3.8-27B`:

```bash
llmnop \
  --url http://localhost:8000/v1 \
  --api chat \
  --api-key token-abc123 \
  --model Qwen/Qwen3.8-27B \
  --input-tokens 550 \
  --output-cap 2048 \
  --requests 10 \
  --concurrency 4
```

This sends 10 requests with up to four in flight. Each completed or failed attempt frees a slot for the next request. Failed attempts count toward the total and are not retried.

Change the URL and model to use your endpoint. `--tokenizer` accepts a Hugging Face tokenizer ID or a local `tokenizer.json`. When omitted, it uses the model name.

All requests stream. The API option selects the path appended to your base URL:

| `--api`          | Path                | Authentication |
| ---------------- | ------------------- | -------------- |
| `chat` (default) | `/chat/completions` | Bearer token   |
| `responses`      | `/responses`        | Bearer token   |
| `messages`       | `/messages`         | `x-api-key`    |

Include the version prefix, such as `/v1`, in `--url`. For authenticated endpoints, pass `--api-key "$API_KEY"`. It is optional for local servers that do not require authentication.

## Read the results

| Question                                  | What to look at                                                                                                       |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| How long before anything arrives?         | **TTFT:** time to the first nonempty content or exposed reasoning.                                                    |
| How long before the answer starts?        | **TTFO:** time to the first nonempty response content.                                                                |
| How long until the request finishes?      | **Request latency:** time from sending the request to stream completion.                                              |
| How fast does generation arrive?          | **Generation rate:** locally counted content and reasoning tokens per second during generation.                       |
| Does the stream stall?                    | **Mean and longest stream-event gaps:** pauses between content or reasoning deliveries.                               |
| How consistent is the experience?         | **Mean and percentiles:** compare the median (p50) with slower requests at p95 and p99.                               |
| How much work does the endpoint complete? | **Completed requests/s and generated tokens/s:** throughput across the measured run.                                  |
| Are requests completing reliably?         | **Completed, failed, timed out, and cancelled counts.** Output-limit and empty completions are identified separately. |

Repeat a workload at different concurrency levels, keeping input sizes, output caps, tokenizer, and reasoning settings consistent. Check actual output lengths when comparing timings.

A few details matter when comparing runs:

- **Only completed measured requests enter the summary statistics.** Check failures and timeouts alongside latency. Warmup is excluded.
- **The API may hide reasoning.** TTFT measures what is delivered, not when the model internally starts generating. No exposed reasoning does not mean no reasoning occurred. These timings do not isolate prefill speed.
- **Streaming events are not individual tokens.** Events can contain several tokens. Generation rate and inter-token latency are estimates over the first-to-last text delivery window; the rate uses `(generated tokens − 1) / window`.
- **Missing measurements stay missing.** For example, reasoning without response content has no TTFO. Unavailable values appear as `—` in the table and `null` in JSON.

## Shape the workload

Prompts are passages sampled from Shakespeare, starting at random token positions and sized to the requested input length. Varying the starting position reduces prefix-cache reuse compared with repeatedly sending the same prompt. This is a synthetic load test, not an evaluation of answer quality or coding ability.

| Option                  | Meaning                             | Default                        |
| ----------------------- | ----------------------------------- | ------------------------------ |
| `--input-tokens`        | Mean prompt-text token target       | `550`                          |
| `--input-tokens-stddev` | Variation in input targets          | `0`                            |
| `--output-cap`          | Mean requested generation-token cap | Omitted; required for Messages |
| `--output-cap-stddev`   | Variation in output caps            | `0`                            |

Zero standard deviation gives every request the same target or cap, and the prompt text still varies. Nonzero values sample positive integer lengths from a normal distribution. Input targets count prompt text using the selected tokenizer, excluding server-added formatting. Varying output caps requires `--output-cap`.

**An output cap is a ceiling, not a promised response length.** A model may finish earlier or spend its allowance reasoning before producing an answer. A server may also ignore unsupported cap fields: llmnop sends `max_completion_tokens` for Chat, `max_output_tokens` for Responses, and `max_tokens` for Messages. Ollama versions that ignore the Chat field can be tested through Responses or Messages instead.

Use `--requests` to set the number of measured attempts (default `10`) and `--concurrency` to limit simultaneous requests (default `1`). Optional `--warmup N` runs N additional attempts before measurement. `--request-timeout` limits each entire request, including its stream, to 600 seconds by default. Ctrl-C cancels the run and saves the results collected so far.

### Reasoning and other request settings

Pass model- or server-specific fields through `--extra-inputs` as one JSON object. For example:

| API       | Example                                                                                   |
| --------- | ----------------------------------------------------------------------------------------- |
| Chat      | `--extra-inputs '{"reasoning_effort":"low"}'`                                             |
| Responses | `--extra-inputs '{"reasoning":{"effort":"low"}}'`                                         |
| Messages  | `--output-cap 2048 --extra-inputs '{"thinking":{"type":"enabled","budget_tokens":1024}}'` |

These fields go directly into every request. Supported settings depend on the model and server; choose an output cap that satisfies their requirements. llmnop does not translate effort levels or adjust caps around a thinking budget.

Extra inputs cannot replace fields controlled by llmnop, such as the model, messages, streaming mode, or output cap. Use `--api-key` for credentials. Extra inputs are saved in the results.

## Save and inspect results

Every run saves two files and prints their location:

- **`summary.json`** — configuration, outcome counts, throughput, and metric statistics.
- **`requests.jsonl`** — one record per attempted request, with timings, token counts, finish reason, provider usage, and any error. Warmup records are marked separately.

Use `--results-dir ./results` to choose where run directories are created. Otherwise, llmnop uses your platform's application data/state directory.

The default stdout format is a table. Use `--format json` for a machine-readable summary or `--format none` to rely on the saved files. Progress and diagnostics go to stderr. Prompt and response text are not saved.

Primary token counts use the selected local tokenizer. Provider-reported usage is saved separately and never substituted into local metrics. `--request-usage` asks Chat endpoints to include optional usage; usage returned by any API is recorded automatically.

Exit codes: `0` for all attempts completed, `1` for request or operational failures, `2` for invalid arguments, and `130` for interruption. Reaching an output cap is a completion, not a failure.

Run `llmnop --help` for the full option list.

## License

[Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0)
