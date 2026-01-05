<p align="center">
  <img src="assets/llmnop.png" alt="llmnop" width="420">
</p>

<p align="center">
  <a href="#installation">Installation</a> | <a href="#usage">Usage</a>
</p>

`llmnop` benchmarks LLM inference endpoints. Point it at any OpenAI-compatible API and get time-to-first-token, inter-token latency, throughput, and end-to-end latency stats.

## Installation

```bash
# Homebrew
brew install jpreagan/tap/llmnop

# Or with the shell installer
curl -sSfL https://github.com/jpreagan/llmnop/releases/latest/download/llmnop-installer.sh | sh
```

The installer places `llmnop` in `~/.local/bin`. Make sure that's on your `PATH`.

## Usage

```bash
export OPENAI_API_BASE=http://localhost:8000/v1
export OPENAI_API_KEY=your-key

llmnop --model deepseek-ai/DeepSeek-R1-Distill-Qwen-7B
```

That's it. By default, llmnop sends two requests with ~550 input tokens each and lets the model generate freely. Results print to stdout and save to `result_outputs/`.

### Options

```
-m, --model <MODEL>                   Model name (required)
    --tokenizer <TOKENIZER>           Hugging Face tokenizer (defaults to model name)
    --max-num-completed-requests <N>  Number of requests [default: 2]
    --num-concurrent-requests <N>     Parallel requests [default: 1]
    --mean-input-tokens <N>           Target input length [default: 550]
    --stddev-input-tokens <N>         Input length variance [default: 150]
    --mean-output-tokens <N>          Target output length [default: none]
    --stddev-output-tokens <N>        Output length variance [default: 10]
    --results-dir <DIR>               Output directory [default: result_outputs]
    --timeout <SECONDS>               Request timeout [default: 600]
    --no-progress                     Hide progress bar
```

### Examples

Run with concurrent requests:

```bash
llmnop --model deepseek-ai/DeepSeek-R1-Distill-Qwen-7B --num-concurrent-requests 10 --max-num-completed-requests 100
```

Constrain output length (for reproducible results):

```bash
llmnop --model deepseek-ai/DeepSeek-R1-Distill-Qwen-7B --mean-output-tokens 150
```

Specify a tokenizer when the served model name doesn't match Hugging Face:

```bash
llmnop --model gpt-oss:20b --tokenizer openai/gpt-oss-20b
```

In some cases, you may want to use a neutral tokenizer when comparing different models:

```bash
llmnop --model gpt-oss:20b --tokenizer hf-internal-testing/llama-tokenizer
```

### Output

llmnop prints stats to stdout and writes two JSON files per run:

- `{model}_{input}_{output}_summary.json` - aggregated stats with percentiles
- `{model}_{input}_{output}_individual_responses.json` - per-request data

## License

[Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0)
