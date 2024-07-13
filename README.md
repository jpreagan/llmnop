# LLMNOP

LLMNOP is a command-line tool for LLMOps used to benchmark Large Language Model (LLM) performance metrics.

## Features

- Measures key performance indicators: TTFT, ~~TPOT~~, ~~Latency~~, and Throughput
- Support for concurrent requests to simulate real-world load
- Configurable input and output tokens distribution for realistic load testing
- Standardizes metrics across models using BPE tokenization

## Installation

```bash
go install github.com/jpreagan/llmnop@latest
```

## Usage

### Benchmark

Benchmark LLM performance metrics like latency and throughput

#### Usage

```
Usage:
  llmnop benchmark [flags]

Flags:
  -k, --api-key string             API key for the inference server
  -u, --base-url string            Base URL for the inference server (e.g., "https://example.com/v1")
  -c, --concurrency int            Number of concurrent requests (default: 1) (default 1)
  -h, --help                       help for benchmark
      --mean-input-tokens int      Mean number of tokens to send in the prompt for the request (default: 550) (default 550)
      --mean-output-tokens int     Mean number of tokens to generate from each LLM request (default: 150) (default 150)
  -m, --model string               Specify the model to benchmark (e.g., "meta-llama/Meta-Llama-3-70B-Instruct")
  -n, --num-iterations int         Number of iterations to run (default: 2) (default 2)
      --stddev-input-tokens int    Standard deviation of number of tokens to send in the prompt for the request (default: 150) (default 150)
      --stddev-output-tokens int   Standard deviation on the number of tokens to generate per LLM request (default: 10) (default 10)
```

#### Example

```bash
llmnop benchmark \
  --base-url https://example.com/v1 \
  --api-key your-api-key-here \
  --model meta-llama/Meta-Llama-3-8B-Instruct \
  --num-iterations 10 \
  --concurrency 1
```

#### Sample Output

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
   - min: 142.769259 ms, max: 264.399014 ms, stddev: 32.770924 ms
   - p25: 180.707262 ms, p50: 195.950439 ms, p75: 213.903887 ms
   - p90: 241.836183 ms, p95: 253.117599 ms, p99: 262.142731 ms
   - mean: 200.360268 ms

2. Throughput Metrics:
   - min: 15.691557 tokens/s, max: 15.787429 tokens/s, stddev: 0.031595 tokens/s
   - p25: 15.718978 tokens/s, p50: 15.729153 tokens/s, p75: 15.761459 tokens/s
   - p90: 15.778008 tokens/s, p95: 15.782718 tokens/s, p99: 15.786487 tokens/s
   - mean: 15.736316 tokens/s

Request Statistics:
- Total Requests: 10
- Successful: 10 (100.00%)
- Failed: 0 (0.00%)
```

## License

Apache-2.0 license

## Contributing

Contributions are welcome! Please feel free to submit a [Pull Request](https://github.com/jpreagan/llmnop/pulls).
