package cmd

import (
	"context"
	"fmt"
	"io"
	"math"
	"math/rand"
	"sort"
	"strings"
	"sync"
	"time"

	openai "github.com/sashabaranov/go-openai"
	"github.com/spf13/cobra"
	"github.com/sugarme/tokenizer"
	"github.com/sugarme/tokenizer/model/bpe"
	"github.com/sugarme/tokenizer/pretokenizer"
	"github.com/sugarme/tokenizer/processor"
)

var (
	baseURL            string
	apiKey             string
	model              string
	numIterations      int
	concurrency        int
	meanInputTokens    int
	stddevInputTokens  int
	meanOutputTokens   int
	stddevOutputTokens int
)

type BenchmarkResult struct {
	TTFT         time.Duration
	InputTokens  int
	OutputTokens int
	Throughput   float64
}

var benchmarkCmd = &cobra.Command{
	Use:   "benchmark",
	Short: "Benchmark LLM performance metrics like latency and throughput",
	Long:  "Benchmark LLM performance metrics like latency and throughput",
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Printf("LLM Benchmark Results for %s\n", model)
		fmt.Printf("Endpoint: %s/chat/completions\n", baseURL)
		fmt.Printf("Iterations: %d\n", numIterations)
		fmt.Printf("Concurrency: %d\n", concurrency)
		fmt.Printf("Mean Input Tokens: %d\n", meanInputTokens)
		fmt.Printf("Stddev Input Tokens: %d\n", stddevInputTokens)
		fmt.Printf("Mean Output Tokens: %d\n", meanOutputTokens)
		fmt.Printf("Stddev Output Tokens: %d\n", stddevOutputTokens)

		var wg sync.WaitGroup
		results := make(chan *BenchmarkResult, numIterations*concurrency)
		var totalRequests, successfulRequests, failedRequests int
		var mu sync.Mutex

		for i := 0; i < concurrency; i++ {
			wg.Add(1)
			go func() {
				defer wg.Done()
				for j := 0; j < numIterations; j++ {
					mu.Lock()
					totalRequests++
					mu.Unlock()

					prompt, inputTokens := getPromptInstance(meanInputTokens, stddevInputTokens, meanOutputTokens, stddevOutputTokens)
					result, err := benchmark(baseURL, apiKey, model, prompt, inputTokens)
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

		ttfts := []float64{}
		throughputs := []float64{}
		for result := range results {
			ttfts = append(ttfts, result.TTFT.Seconds()*1000) // convert to milliseconds
			throughputs = append(throughputs, result.Throughput)
		}

		printTTFTMetrics(ttfts)
		printThroughputMetrics(throughputs)

		fmt.Println("\nRequest Statistics:")
		fmt.Printf("- Total Requests: %d\n", totalRequests)
		fmt.Printf("- Successful: %d (%.2f%%)\n", successfulRequests, float64(successfulRequests)/float64(totalRequests)*100)
		fmt.Printf("- Failed: %d (%.2f%%)\n", failedRequests, float64(failedRequests)/float64(totalRequests)*100)
	},
}

func init() {
	rootCmd.AddCommand(benchmarkCmd)

	benchmarkCmd.Flags().StringVarP(&baseURL, "base-url", "u", "", "Base URL for the inference server (e.g., \"https://example.com/v1\")")
	benchmarkCmd.Flags().StringVarP(&apiKey, "api-key", "k", "", "API key for the inference server")
	benchmarkCmd.Flags().StringVarP(&model, "model", "m", "", "Specify the model to benchmark (e.g., \"meta-llama/Meta-Llama-3-70B-Instruct\")")
	benchmarkCmd.Flags().IntVarP(&numIterations, "num-iterations", "n", 2, "Number of iterations to run")
	benchmarkCmd.Flags().IntVarP(&concurrency, "concurrency", "c", 1, "Number of concurrent requests")
	benchmarkCmd.Flags().IntVarP(&meanInputTokens, "mean-input-tokens", "", 550, "Mean number of tokens to send in the prompt for the request")
	benchmarkCmd.Flags().IntVarP(&stddevInputTokens, "stddev-input-tokens", "", 150, "Standard deviation of number of tokens to send in the prompt for the request")
	benchmarkCmd.Flags().IntVarP(&meanOutputTokens, "mean-output-tokens", "", 150, "Mean number of tokens to generate from each LLM request")
	benchmarkCmd.Flags().IntVarP(&stddevOutputTokens, "stddev-output-tokens", "", 10, "Standard deviation on the number of tokens to generate per LLM request")

	benchmarkCmd.MarkFlagRequired("base-url")
	benchmarkCmd.MarkFlagRequired("model")
}

func getByteLevel(addPrefixSpace bool, trimOffsets bool) *tokenizer.Tokenizer {
	vocabFile := "data/gpt2-vocab.json"
	mergeFile := "data/gpt2-merges.txt"

	model, err := bpe.NewBpeFromFiles(vocabFile, mergeFile)
	if err != nil {
		panic(fmt.Sprintf("Failed to load BPE model: %v", err))
	}

	tk := tokenizer.NewTokenizer(model)

	pretok := pretokenizer.NewByteLevel()
	pretok.SetAddPrefixSpace(addPrefixSpace)
	pretok.SetTrimOffsets(trimOffsets)
	tk.WithPreTokenizer(pretok)

	pprocessor := processor.NewByteLevelProcessing(pretok)
	tk.WithPostProcessor(pprocessor)

	return tk
}

func benchmark(baseURL, apiKey, model, prompt string, inputTokens int) (*BenchmarkResult, error) {
	config := openai.DefaultConfig(apiKey)
	config.BaseURL = baseURL

	client := openai.NewClientWithConfig(config)
	ctx := context.Background()

	req := openai.ChatCompletionRequest{
		Model: model,
		Messages: []openai.ChatCompletionMessage{
			{Role: "system", Content: "You are a helpful assistant."},
			{Role: "user", Content: prompt},
		},
		Stream: true,
	}

	start := time.Now()
	stream, err := client.CreateChatCompletionStream(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("error creating chat completion stream: %v", err)
	}
	defer stream.Close()

	var ttft time.Duration
	var firstTokenReceived bool
	var outputTokens int

	for {
		response, err := stream.Recv()
		if err != nil {
			if err == io.EOF {
				break
			}
			return nil, fmt.Errorf("error receiving stream response: %v", err)
		}

		if len(response.Choices) > 0 && response.Choices[0].Delta.Content != "" {
			outputTokens++
			if !firstTokenReceived {
				ttft = time.Since(start)
				firstTokenReceived = true
			}
		}
	}

	if !firstTokenReceived {
		return nil, fmt.Errorf("no tokens received")
	}

	end := time.Now()
	throughput := float64(outputTokens) / end.Sub(start).Seconds()

	return &BenchmarkResult{TTFT: ttft, InputTokens: inputTokens, OutputTokens: outputTokens, Throughput: throughput}, nil
}

func getPromptInstance(meanInputTokens, stddevInputTokens, meanOutputTokens, stddevOutputTokens int) (string, int) {
	sonnetLines := []string{
		"Shall I compare thee to a summer's day?",
		"Thou art more lovely and more temperate:",
		"Rough winds do shake the darling buds of May,",
		"And summer's lease hath all too short a date:",
		"Sometime too hot the eye of heaven shines,",
		"And often is his gold complexion dimm'd;",
		"And every fair from fair sometime declines,",
		"By chance or nature's changing course untrimm'd;",
		"But thy eternal summer shall not fade",
		"Nor lose possession of that fair thou owest;",
		"Nor shall Death brag thou wander'st in his shade,",
		"When in eternal lines to time thou growest:",
		"So long as men can breathe or eyes can see,",
		"So long lives this and this gives life to thee.",
		"Then let not winter's ragged hand deface",
		"In thee thy summer, ere thou be distill'd:",
		"Make sweet some vial; treasure thou some place",
		"With beauty's treasure, ere it be self-kill'd.",
		"That use is not forbidden usury,",
		"Which happies those that pay the willing loan;",
		"That's for thyself to breed another thee,",
		"Or ten times happier, be it ten for one;",
		"Ten times thyself were happier than thou art,",
		"If ten of thine ten times refigured thee:",
		"Then what could death do, if thou shouldst depart,",
		"Leaving thee living in posterity?",
		"Be not self-will'd, for thou art much too fair",
		"To be death's conquest and make worms thine heir.",
		"Where art thou, Muse, that thou forget'st so long",
		"To speak of that which gives thee all thy might?",
		"Spend'st thou thy fury on some worthless song,",
		"Darkening thy power to lend base subjects light?",
		"Return, forgetful Muse, and straight redeem",
		"In gentle numbers time so idly spent;",
		"Sing to the ear that doth thy lays esteem",
		"And gives thy pen both skill and argument.",
		"Rise, resty Muse, my love's sweet face survey,",
		"If Time have any wrinkle graven there;",
		"If any, be a satire to decay,",
		"And make Time's spoils despised every where.",
		"Give my love fame faster than Time wastes life;",
		"So thou prevent'st his scythe and crooked knife.",
		"My glass shall not persuade me I am old,",
		"So long as youth and thou are of one date;",
		"But when in thee time's furrows I behold,",
		"Then look I death my days should expiate.",
		"For all that beauty that doth cover thee",
		"Is but the seemly raiment of my heart,",
		"Which in thy breast doth live, as thine in me:",
		"How can I then be elder than thou art?",
		"O, therefore, love, be of thyself so wary",
		"As I, not for myself, but for thee will;",
		"Bearing thy heart, which I will keep so chary",
		"As tender nurse her babe from faring ill.",
		"Presume not on thy heart when mine is slain;",
		"Thou gavest me thine, not to give back again.",
		"So am I as the rich, whose blessed key",
		"Can bring him to his sweet up-locked treasure,",
		"The which he will not every hour survey,",
		"For blunting the fine point of seldom pleasure.",
		"Therefore are feasts so solemn and so rare,",
		"Since, seldom coming, in the long year set,",
		"Like stones of worth they thinly placed are,",
		"Or captain jewels in the carcanet.",
		"So is the time that keeps you as my chest,",
		"Or as the wardrobe which the robe doth hide,",
		"To make some special instant special blest,",
		"By new unfolding his imprison'd pride.",
		"Blessed are you, whose worthiness gives scope,",
		"Being had, to triumph, being lack'd, to hope.",
		"If there be nothing new, but that which is",
		"Hath been before, how are our brains beguiled,",
		"Which, labouring for invention, bear amiss",
		"The second burden of a former child!",
		"O, that record could with a backward look,",
		"Even of five hundred courses of the sun,",
		"Show me your image in some antique book,",
		"Since mind at first in character was done!",
		"That I might see what the old world could say",
		"To this composed wonder of your frame;",
		"Whether we are mended, or whether better they,",
		"Or whether revolution be the same.",
		"O, sure I am, the wits of former days",
		"To subjects worse have given admiring praise.",
	}

	tk := getByteLevel(true, false) // Create a new instance of the tokenizer

	numInputTokens := sampleRandomPositiveInt(meanInputTokens, stddevInputTokens)
	numOutputTokens := sampleRandomPositiveInt(meanOutputTokens, stddevOutputTokens)
	prompt := strings.Builder{}
	prompt.WriteString(fmt.Sprintf("Randomly stream lines from the following text with %d output tokens. Don't generate eos tokens:\n\n", numOutputTokens))

	inputSeq := tokenizer.NewInputSequence(prompt.String())
	encoding, _ := tk.Encode(tokenizer.NewSingleEncodeInput(inputSeq), false)
	tokenCount := len(encoding.GetIds())

	for tokenCount < numInputTokens {
		line := sonnetLines[rand.Intn(len(sonnetLines))]
		prompt.WriteString(line + "\n")
		lineSeq := tokenizer.NewInputSequence(line)
		lineEncoding, _ := tk.Encode(tokenizer.NewSingleEncodeInput(lineSeq), false)
		tokenCount += len(lineEncoding.GetIds())
	}

	return prompt.String(), tokenCount
}

func sampleRandomPositiveInt(mean, stddev int) int {
	ret := -1
	for ret <= 0 {
		ret = int(rand.NormFloat64()*float64(stddev) + float64(mean))
	}
	return ret
}

func printTTFTMetrics(ttfts []float64) {
	if len(ttfts) == 0 {
		fmt.Println("No successful requests.")
		return
	}

	sort.Float64s(ttfts)

	meanTTFT := mean(ttfts)
	p25 := percentile(ttfts, 25)
	p50 := percentile(ttfts, 50)
	p75 := percentile(ttfts, 75)
	p90 := percentile(ttfts, 90)
	p95 := percentile(ttfts, 95)
	p99 := percentile(ttfts, 99)
	minTTFT := ttfts[0]
	maxTTFT := ttfts[len(ttfts)-1]
	stddevTTFT := stddev(ttfts, meanTTFT)

	fmt.Println("\nPerformance Metrics:")
	fmt.Println("1. Time To First Token (TTFT):")
	fmt.Printf("   - min: %.6f ms, max: %.6f ms, stddev: %.6f ms\n", minTTFT, maxTTFT, stddevTTFT)
	fmt.Printf("   - p25: %.6f ms, p50: %.6f ms, p75: %.6f ms\n", p25, p50, p75)
	fmt.Printf("   - p90: %.6f ms, p95: %.6f ms, p99: %.6f ms\n", p90, p95, p99)
	fmt.Printf("   - mean: %.6f ms\n", meanTTFT)
}

func printThroughputMetrics(throughputs []float64) {
	if len(throughputs) == 0 {
		fmt.Println("No successful requests.")
		return
	}

	sort.Float64s(throughputs)

	meanThroughput := mean(throughputs)
	p25 := percentile(throughputs, 25)
	p50 := percentile(throughputs, 50)
	p75 := percentile(throughputs, 75)
	p90 := percentile(throughputs, 90)
	p95 := percentile(throughputs, 95)
	p99 := percentile(throughputs, 99)
	minThroughput := throughputs[0]
	maxThroughput := throughputs[len(throughputs)-1]
	stddevThroughput := stddev(throughputs, meanThroughput)

	fmt.Println("\n2. Throughput Metrics:")
	fmt.Printf("   - min: %.6f tokens/s, max: %.6f tokens/s, stddev: %.6f tokens/s\n", minThroughput, maxThroughput, stddevThroughput)
	fmt.Printf("   - p25: %.6f tokens/s, p50: %.6f tokens/s, p75: %.6f tokens/s\n", p25, p50, p75)
	fmt.Printf("   - p90: %.6f tokens/s, p95: %.6f tokens/s, p99: %.6f tokens/s\n", p90, p95, p99)
	fmt.Printf("   - mean: %.6f tokens/s\n", meanThroughput)
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
