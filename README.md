<!-- Centered version -->
<p align="center">
  <img src="assets/llmnop.png" alt="llmnop" width="420">
</p>

<p align="center">
  <a href="#installation">Installation</a> | <a href="#usage">Usage</a>
</p>

`llmnop` is a command-line tool for benchmarking the performance of Large Language Models (LLM) inference endpoints that are compatible with the OpenAI API. It measures key performance metrics like time to first token (TTFT), inter-token latency, and overall throughput under concurrent loads.

## Features

- **Streaming Metrics**: Accurately measures performance for streaming responses.
- **Concurrent Benchmarking**: Send multiple requests in parallel to simulate real-world load.
- **Detailed Performance Metrics**:
  - Time To First Token (TTFT)
  - Inter-Token Latency (average time between subsequent tokens)
  - End-to-end Request Latency
  - Throughput (tokens/second)
- **Realistic Workload Simulation**: Generates prompts with variable input and output token lengths based on a normal distribution.
- **Detailed JSON Output**: Saves detailed per-request data and a final summary report.
- **Tokenizer-Aware**: Uses Hugging Face `tokenizers` to count tokens for prompt generation and metric calculation.

## Installation

### 1. Use the installer script (recommended)

The installer script will download and install the correct binary for your architecture and platform:

```bash
curl -sSfL https://github.com/jpreagan/llmnop/releases/latest/download/llmnop-installer.sh | sh
```

### 2. Download a precompiled binary

Grab the tarball for your platform from the Releases page, extract the binary, and place it somewhere on your PATH.

### 3. Build from source

```bash
git clone https://github.com/jpreagan/llmnop.git
cd llmnop
cargo build --release
```

## Usage

```
llmnop [OPTIONS] --model <MODEL>
```

### Options

```
-m, --model <MODEL>
    --max-num-completed-requests <MAX_NUM_COMPLETED_REQUESTS>  [default: 1]
    --num-concurrent-requests <NUM_CONCURRENT_REQUESTS>        [default: 1]
    --mean-input-tokens <MEAN_INPUT_TOKENS>                    [default: 550]
    --stddev-input-tokens <STDDEV_INPUT_TOKENS>                [default: 150]
    --mean-output-tokens <MEAN_OUTPUT_TOKENS>                  [default: 150]
    --stddev-output-tokens <STDDEV_OUTPUT_TOKENS>              [default: 10]
    --results-dir <RESULTS_DIR>                                [default: result_outputs]
    --timeout <TIMEOUT>                                        [default: 600]
-h, --help                                                     Print help
-V, --version                                                  Print version
```

### Example

```bash
export OPENAI_API_BASE=http://localhost:8000
export OPENAI_API_KEY=token-abc123

llmnop \
    --model "Qwen/Qwen3-4B" \
    --mean-input-tokens 550 \
    --stddev-input-tokens 150 \
    --mean-output-tokens 150 \
    --stddev-output-tokens 10 \
    --max-num-completed-requests 2 \
    --timeout 600 \
    --num-concurrent-requests 1 \
    --results-dir "result_outputs"
```

## License

This project is licensed under the [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0).
