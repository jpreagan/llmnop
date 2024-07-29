# LLMNOP

LLMNOP is a command-line tool for benchmarking Large Language Models (LLM) performance metrics.

## Features

- Measures key performance indicators: Time To First Token (TTFT) and Throughput
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
  -u, --base-url string            base URL for the inference server (e.g., "https://example.com/v1")
  -c, --concurrency int            number of concurrent requests (default 1)
  -h, --help                       help for llmnop
      --mean-input-tokens int      mean number of tokens to send in the prompt for the request (default 550)
      --mean-output-tokens int     mean number of tokens to generate from each LLM request (default 150)
  -m, --model string               specify the model to benchmark (e.g., "meta-llama/Meta-Llama-3-70B-Instruct")
  -n, --num-iterations int         number of iterations to run (default 2)
      --stddev-input-tokens int    standard deviation of number of tokens to send in the prompt for the request (default 150)
      --stddev-output-tokens int   standard deviation on the number of tokens to generate per LLM request (default 10)
  -t, --tokenizer string           path to the tokenizer.json file
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
Benchmark Setup
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Model: meta-llama/Meta-Llama-3-70B-Instruct
Endpoint: https://example.com/v1/chat/completions
Total Requests: 10 (Iterations: 10, Concurrency: 1)
Input Tokens: Mean 550 ± 150
Output Tokens: Mean 150 ± 10
Timestamp: 2024-07-28T16:11:43-10:00

Performance Metrics
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

1. Time To First Token (TTFT) (ms):
       [───────────────|───────────────|───────────────|───────────────]
      Min             P25        Median (P50)         P75             Max
    106 ms          123 ms          129 ms          177 ms          196 ms

   Average (Mean): 145 ms
   Standard Deviation: 32 ms

2. Throughput (tokens/second):
       [───────────────|───────────────|───────────────|───────────────]
      Min             P25        Median (P50)         P75             Max
   17.9 t/s        18.2 t/s        18.8 t/s        19.1 t/s        19.4 t/s

   Average (Mean): 18.7 t/s
   Standard Deviation: 0.5 t/s

3. Input Token Count:
       [───────────────|───────────────|───────────────|───────────────]
      Min             P25        Median (P50)         P75             Max
      204             327             450             566             683

   Average (Mean): 446
   Standard Deviation: 196

4. Output Token Count:
       [───────────────|───────────────|───────────────|───────────────]
      Min             P25        Median (P50)         P75             Max
      160             238             317             586             854

   Average (Mean): 444
   Standard Deviation: 297

Request Summary
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Total Requests:    10
Successful:        10 (100.00%)
Failed:            0 (0.00%)
```

## License

Apache-2.0 license

## Contributing

Contributions are welcome! Please feel free to submit a [Pull Request](https://github.com/jpreagan/llmnop/pulls).
