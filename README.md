# LLMNOP

LLMNOP is a command-line tool for LLMOps used to benchmark Large Language Model (LLM) performance metrics like throughput and latency.

## Features

- Measures key performance indicators: TTFT, ~~TPOT~~, ~~Latency~~, and Throughput
- Support for concurrent requests to simulate real-world load

## Installation

```bash
go install github.com/jpreagan/llmnop@latest
```

## Usage

### Benchmark

```bash
llmnop benchmark [options]
```

#### Options

- --base-url or -u: URL of the inference server (e.g., "https://example.com/v1")
- --api-key or -k: API key for authentication with the inference server
- --model or -m: Specify the model to benchmark (e.g., "meta-llama/Meta-Llama-3-70B-Instruct")
- --iterations or -n: Number of iterations to run (default: 10)
- --concurrent or -c: Number of concurrent requests (default: 1)
- --max-tokens or -t: Maximum number of tokens to generate (default: 100)
- --prompt or -p: The prompt to be used for benchmarking

#### Example

```bash
llmnop benchmark \
  --base-url https://example.com/v1 \
  --api-key your-api-key-here \
  --model meta-llama/Meta-Llama-3-70B-Instruct \
  --prompt "Explain machine learning in simple terms."
```

#### Sample Output

```
LLM Benchmark Results for meta-llama/Meta-Llama-3-70B-Instruct
Endpoint: https://example.com/v1/chat/completions
Iterations: 10
Concurrency: 1
Output Length: 100 tokens

Performance Metrics:
1. Time To First Token (TTFT):
   - Average: 75.021 ms
   - p50: 74.866 ms, p90: 75.486 ms, p99: 76.683 ms
   - min: 74.206 ms, max: 76.816 ms, stddev: 0.704 ms

Request Statistics:
- Total Requests: 10
- Successful: 10 (100.00%)
- Failed: 0 (0.00%)
```

## License

Apache-2.0 license

## Contributing

Contributions are welcome! Please feel free to submit a [Pull Request](https://github.com/jpreagan/llmnop/pulls).
