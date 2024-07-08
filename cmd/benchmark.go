package cmd

import (
	"bufio"
	"encoding/json"
	"fmt"
	"math"
	"net/http"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/spf13/cobra"
)

var (
	baseURL    string
	apiKey     string
	model      string
	iterations int
	concurrent int
	maxTokens  int
	prompt     string
)

type BenchmarkResult struct {
	TTFT time.Duration
}

type ChatCompletionChunk struct {
	ID      string `json:"id"`
	Object  string `json:"object"`
	Created int64  `json:"created"`
	Model   string `json:"model"`
	Choices []struct {
		Index        int    `json:"index"`
		Delta        Delta  `json:"delta"`
		FinishReason string `json:"finish_reason"`
	} `json:"choices"`
}

type Delta struct {
	Role    string `json:"role,omitempty"`
	Content string `json:"content,omitempty"`
}

// benchmarkCmd represents the benchmark command
var benchmarkCmd = &cobra.Command{
	Use:   "benchmark",
	Short: "Benchmark performance metrics like latency and throughput for an LLM",
	Long:  "Benchmark performance metrics like latency and throughput for an LLM",
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Printf("LLM Benchmark Results for %s\n", model)
		fmt.Printf("Endpoint: %s/chat/completions\n", baseURL)
		fmt.Printf("Iterations: %d\n", iterations)
		fmt.Printf("Concurrency: %d\n", concurrent)
		fmt.Printf("Output Length: %d tokens\n", maxTokens)

		var wg sync.WaitGroup
		results := make(chan *BenchmarkResult, iterations*concurrent)
		var totalRequests, successfulRequests, failedRequests int
		var mu sync.Mutex

		for i := 0; i < concurrent; i++ {
			wg.Add(1)
			go func() {
				defer wg.Done()
				for j := 0; j < iterations; j++ {
					mu.Lock()
					totalRequests++
					mu.Unlock()

					result, err := runBenchmark(baseURL, apiKey, model, prompt, maxTokens)
					if err != nil {
						fmt.Printf("Error running benchmark: %v\n", err)
						mu.Lock()
						failedRequests++
						mu.Unlock()
						continue
					}
					results <- result

					mu.Lock()
					successfulRequests++
					mu.Unlock()
				}
			}()
		}

		wg.Wait()
		close(results)

		// Collect TTFTs
		ttfts := []float64{}
		for result := range results {
			ttfts = append(ttfts, result.TTFT.Seconds()*1000) // convert to milliseconds
		}

		printTTFTMetrics(ttfts)

		fmt.Println("\nRequest Statistics:")
		fmt.Printf("- Total Requests: %d\n", totalRequests)
		fmt.Printf("- Successful: %d (%.2f%%)\n", successfulRequests, float64(successfulRequests)/float64(totalRequests)*100)
		fmt.Printf("- Failed: %d (%.2f%%)\n", failedRequests, float64(failedRequests)/float64(totalRequests)*100)
	},
}

func init() {
	rootCmd.AddCommand(benchmarkCmd)

	benchmarkCmd.Flags().StringVarP(&baseURL, "base-url", "u", "", "URL of the inference server (e.g., \"https://example.com/v1\")")
	benchmarkCmd.Flags().StringVarP(&apiKey, "api-key", "k", "", "API key for authentication with the inference server")
	benchmarkCmd.Flags().StringVarP(&model, "model", "m", "", "Specify the model to benchmark (e.g., \"meta-llama/Meta-Llama-3-70B-Instruct\")")
	benchmarkCmd.Flags().IntVarP(&iterations, "iterations", "n", 100, "Number of iterations to run (default: 10)")
	benchmarkCmd.Flags().IntVarP(&concurrent, "concurrent", "c", 1, "Number of concurrent requests (default: 1)")
	benchmarkCmd.Flags().IntVarP(&maxTokens, "max-tokens", "t", 100, "Maximum number of tokens to generate (default: 100)")
	benchmarkCmd.Flags().StringVarP(&prompt, "prompt", "p", "", "The prompt to be used for benchmarking")

	benchmarkCmd.MarkFlagRequired("base-url")
	benchmarkCmd.MarkFlagRequired("api-key")
	benchmarkCmd.MarkFlagRequired("model")
}

func runBenchmark(baseURL, apiKey, model, prompt string, maxTokens int) (*BenchmarkResult, error) {
	url := fmt.Sprintf("%s/chat/completions", baseURL)
	payload := fmt.Sprintf(`{
		"model": "%s",
		"messages": [
			{
				"role": "system",
				"content": "You are a helpful assistant."
			},
			{
				"role": "user",
				"content": "%s"
			}
		],
		"max_tokens": %d,
		"stream": true,
		"stream_options": {
			"include_usage": true
		}
	}`, model, prompt, maxTokens)

	req, err := http.NewRequest("POST", url, strings.NewReader(payload))
	if err != nil {
		return nil, fmt.Errorf("error creating request: %v", err)
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", apiKey))

	start := time.Now()
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("error sending request: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("unexpected status code: %v", resp.StatusCode)
	}

	scanner := bufio.NewScanner(resp.Body)
	var ttft time.Duration
	var firstTokenReceived bool
	for scanner.Scan() {
		line := scanner.Text()
		if strings.HasPrefix(line, "data: ") {
			data := strings.TrimPrefix(line, "data: ")
			if data == "[DONE]" {
				break
			}

			var chunk ChatCompletionChunk
			if err := json.Unmarshal([]byte(data), &chunk); err != nil {
				fmt.Printf("Error unmarshaling chunk: %v\n", err)
				continue
			}

			if !firstTokenReceived && len(chunk.Choices) > 0 && chunk.Choices[0].Delta.Content != "" {
				ttft = time.Since(start)
				firstTokenReceived = true
			}
		}
	}

	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("error reading response: %v", err)
	}

	if !firstTokenReceived {
		return nil, fmt.Errorf("no tokens received")
	}

	return &BenchmarkResult{TTFT: ttft}, nil
}

func printTTFTMetrics(ttfts []float64) {
	if len(ttfts) == 0 {
		fmt.Println("No successful requests.")
		return
	}

	sort.Float64s(ttfts)

	meanTTFT := mean(ttfts)
	p50 := percentile(ttfts, 50)
	p90 := percentile(ttfts, 90)
	p99 := percentile(ttfts, 99)
	minTTFT := ttfts[0]
	maxTTFT := ttfts[len(ttfts)-1]
	stddevTTFT := stddev(ttfts, meanTTFT)

	fmt.Println("\nPerformance Metrics:")
	fmt.Println("1. Time To First Token (TTFT):")
	fmt.Printf("   - Average: %.3f ms\n", meanTTFT)
	fmt.Printf("   - p50: %.3f ms, p90: %.3f ms, p99: %.3f ms\n", p50, p90, p99)
	fmt.Printf("   - min: %.3f ms, max: %.3f ms, stddev: %.3f ms\n", minTTFT, maxTTFT, stddevTTFT)
}

func mean(values []float64) float64 {
	sum := 0.0
	for _, v := range values {
		sum += v
	}
	return sum / float64(len(values))
}

func percentile(data []float64, perc int) float64 {
	index := float64(perc) / 100.0 * float64(len(data)-1)
	if index == float64(int(index)) {
		return data[int(index)]
	}
	i := int(index)
	f := index - float64(i)
	return data[i]*(1-f) + data[i+1]*f
}

func stddev(values []float64, mean float64) float64 {
	sumOfSquares := 0.0
	for _, v := range values {
		sumOfSquares += (v - mean) * (v - mean)
	}
	return math.Sqrt(sumOfSquares / float64(len(values)))
}
