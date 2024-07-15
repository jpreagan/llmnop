# LLMNOP

llmnop is a command-line tool for benchmarking Large Language Models (LLM) performance metrics.

## Features

- Measures key performance indicators: TTFT, ~~TPOT~~, ~~Latency~~, and Throughput
- Support for concurrent requests to simulate real-world load
- Configurable input and output tokens distribution for realistic load testing
- Standardizes comparisons across models using the tokenizer of your choice

## Installation

```bash
go install github.com/jpreagan/llmnop@latest
```

## Usage

```
Usage:
  llmnop [flags]

Flags:
  -k, --api-key string             API key for the inference server
  -u, --base-url string            Base URL for the inference server (e.g., "https://example.com/v1")
  -c, --concurrency int            Number of concurrent requests (default 1)
  -h, --help                       help for llmnop
      --mean-input-tokens int      Mean number of tokens to send in the prompt for the request (default 550)
      --mean-output-tokens int     Mean number of tokens to generate from each LLM request (default 150)
  -m, --model string               Specify the model to benchmark (e.g., "meta-llama/Meta-Llama-3-70B-Instruct")
  -n, --num-iterations int         Number of iterations to run (default 2)
      --stddev-input-tokens int    Standard deviation of number of tokens to send in the prompt for the request (default 150)
      --stddev-output-tokens int   Standard deviation on the number of tokens to generate per LLM request (default 10)
  -t, --tokenizer string           Path to the tokenizer.json file
```

## Example

```bash
llmnop \
  --base-url https://example.com/v1 \
  --api-key your-api-key-here \
  --model meta-llama/Meta-Llama-3-70B-Instruct \
  --num-iterations 10 \
  --concurrency 1
  --tokenizer path/to/tokenizer.json
```

## Sample Output

```
LLM Benchmark Results for meta-llama/Meta-Llama-3-70B-Instruct
Endpoint: https://example.com/v1/chat/completions
Iterations: 10
Concurrency: 1
Mean Input Tokens: 550
Stddev Input Tokens: 150
Mean Output Tokens: 150
Stddev Output Tokens: 10

Performance Metrics:
1. Time To First Token (TTFT):
   - min: 106.419946 ms, p25: 122.582608 ms, p50: 128.821281 ms
   - p75: 177.126659 ms, p90: 189.810643 ms, p95: 193.101382 ms
   - p99: 195.733973 ms, max: 196.392121 ms, stddev: 31.741853 ms
   - mean: 144.805974 ms

2. Throughput Metrics:
   - min: 17.862783 tokens/s, p25: 18.157015 tokens/s, p50: 18.756607 tokens/s
   - p75: 19.139892 tokens/s, p90: 19.320098 tokens/s, p95: 19.376445 tokens/s
   - p99: 19.421523 tokens/s, max: 19.432793 tokens/s, stddev: 0.546217 tokens/s
   - mean: 18.677270 tokens/s

Request Statistics:
- Total Requests: 10
- Successful: 10 (100.00%)
- Failed: 0 (0.00%)
```

## License

Apache-2.0 license

## Contributing

Contributions are welcome! Please feel free to submit a [Pull Request](https://github.com/jpreagan/llmnop/pulls).
