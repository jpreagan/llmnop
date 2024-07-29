package cmd

import (
	"context"
	"fmt"
	"io"
	"math"
	"math/rand"
	"os"
	"sort"
	"strings"
	"sync"
	"time"

	openai "github.com/sashabaranov/go-openai"
	"github.com/spf13/cobra"
	"github.com/sugarme/tokenizer"
	"github.com/sugarme/tokenizer/pretrained"
)

type TimeProvider interface {
	Now() time.Time
	Since(time.Time) time.Duration
}

type SystemTime struct{}

func (s *SystemTime) Now() time.Time {
	return time.Now()
}

func (s *SystemTime) Since(t time.Time) time.Duration {
	return time.Since(t)
}

type BenchmarkResult struct {
	TTFT         time.Duration
	InputTokens  int
	OutputTokens int
	Throughput   float64
}

type BenchmarkSetup struct {
	Model              string
	BaseURL            string
	NumIterations      int
	Concurrency        int
	MeanInputTokens    int
	StddevInputTokens  int
	MeanOutputTokens   int
	StddevOutputTokens int
}

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
	tokenizerPath      string
)

var rootCmd = &cobra.Command{
	Use:   "llmnop",
	Short: "llmnop is a command-line tool for benchmarking Large Language Models (LLM) performance metrics.",
	Long:  "llmnop is a command-line tool for benchmarking Large Language Models (LLM) performance metrics.",
	Run: func(cmd *cobra.Command, args []string) {
		setup := BenchmarkSetup{
			Model:              model,
			BaseURL:            baseURL,
			NumIterations:      numIterations,
			Concurrency:        concurrency,
			MeanInputTokens:    meanInputTokens,
			StddevInputTokens:  stddevInputTokens,
			MeanOutputTokens:   meanOutputTokens,
			StddevOutputTokens: stddevOutputTokens,
		}

		printBenchmarkSetup(setup)

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

					prompt, inputTokens := getPrompt(meanInputTokens, stddevInputTokens, meanOutputTokens, stddevOutputTokens)
					result, err := benchmark(baseURL, apiKey, model, prompt, inputTokens, &SystemTime{})
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
		inputTokensList := []int{}
		outputTokensList := []int{}

		for result := range results {
			ttfts = append(ttfts, result.TTFT.Seconds()*1000) // convert to milliseconds
			throughputs = append(throughputs, result.Throughput)
			inputTokensList = append(inputTokensList, result.InputTokens)
			outputTokensList = append(outputTokensList, result.OutputTokens)
		}

		printMetricsOverview(ttfts, throughputs, inputTokensList, outputTokensList)
		printRequestSummary(totalRequests, successfulRequests, failedRequests)
	},
}

func Execute() {
	err := rootCmd.Execute()
	if err != nil {
		os.Exit(1)
	}
}

func init() {
	rootCmd.Flags().StringVarP(&baseURL, "base-url", "u", "", "base URL for the inference server (e.g., \"https://example.com/v1\")")
	rootCmd.Flags().StringVarP(&apiKey, "api-key", "k", "", "API key for the inference server")
	rootCmd.Flags().StringVarP(&model, "model", "m", "", "specify the model to benchmark (e.g., \"meta-llama/Meta-Llama-3-70B-Instruct\")")
	rootCmd.Flags().IntVarP(&numIterations, "num-iterations", "n", 2, "number of iterations to run")
	rootCmd.Flags().IntVarP(&concurrency, "concurrency", "c", 1, "number of concurrent requests")
	rootCmd.Flags().IntVar(&meanInputTokens, "mean-input-tokens", 550, "mean number of tokens to send in the prompt for the request")
	rootCmd.Flags().IntVar(&stddevInputTokens, "stddev-input-tokens", 150, "standard deviation of number of tokens to send in the prompt for the request")
	rootCmd.Flags().IntVar(&meanOutputTokens, "mean-output-tokens", 150, "mean number of tokens to generate from each LLM request")
	rootCmd.Flags().IntVar(&stddevOutputTokens, "stddev-output-tokens", 10, "standard deviation on the number of tokens to generate per LLM request")
	rootCmd.Flags().StringVarP(&tokenizerPath, "tokenizer", "t", "", "path to the tokenizer.json file")

	rootCmd.MarkFlagRequired("base-url")
	rootCmd.MarkFlagRequired("model")
	rootCmd.MarkFlagRequired("tokenizer")
}

func printBenchmarkSetup(setup BenchmarkSetup) {
	fmt.Println("Benchmark Setup")
	fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
	fmt.Printf("Model: %s\n", setup.Model)
	fmt.Printf("Endpoint: %s/chat/completions\n", setup.BaseURL)
	fmt.Printf("Total Requests: %d (Iterations: %d, Concurrency: %d)\n", setup.NumIterations*setup.Concurrency, setup.NumIterations, setup.Concurrency)
	fmt.Printf("Input Tokens: Mean %d ± %d\n", setup.MeanInputTokens, setup.StddevInputTokens)
	fmt.Printf("Output Tokens: Mean %d ± %d\n", setup.MeanOutputTokens, setup.StddevOutputTokens)
	fmt.Printf("Timestamp: %s\n", time.Now().Format(time.RFC3339))
	fmt.Println()
}

func benchmark(baseURL, apiKey, model, prompt string, inputTokens int, tp TimeProvider) (*BenchmarkResult, error) {
	config := openai.DefaultConfig(apiKey)
	config.BaseURL = baseURL

	client := openai.NewClientWithConfig(config)
	ctx := context.Background()

	req := openai.ChatCompletionRequest{
		Model: model,
		Messages: []openai.ChatCompletionMessage{
			{Role: "system", Content: ""},
			{Role: "user", Content: prompt},
		},
		Stream: true,
	}

	start := tp.Now()
	stream, err := client.CreateChatCompletionStream(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("error creating chat completion stream: %v", err)
	}
	defer stream.Close()

	var ttft time.Duration
	var firstTokenReceived bool
	var generatedText strings.Builder

	for {
		response, err := stream.Recv()
		if err != nil {
			if err == io.EOF {
				break
			}
			return nil, fmt.Errorf("error receiving stream response: %v", err)
		}

		if len(response.Choices) > 0 && response.Choices[0].Delta.Content != "" {
			generatedText.WriteString(response.Choices[0].Delta.Content)
			if !firstTokenReceived {
				ttft = tp.Since(start)
				firstTokenReceived = true
			}
		}
	}

	if !firstTokenReceived {
		return nil, fmt.Errorf("no tokens received")
	}

	end := tp.Now()
	outputTokens := countOutputTokens(generatedText.String())
	elapsedTime := end.Sub(start).Seconds()
	throughput := float64(outputTokens) / elapsedTime

	return &BenchmarkResult{TTFT: ttft, InputTokens: inputTokens, OutputTokens: outputTokens, Throughput: throughput}, nil
}

func getTokenizer() (*tokenizer.Tokenizer, error) {
	if tokenizerPath == "" {
		return nil, fmt.Errorf("tokenizer path not provided")
	}

	tk, err := pretrained.FromFile(tokenizerPath)
	if err != nil {
		return nil, fmt.Errorf("failed to load tokenizer: %v", err)
	}

	return tk, nil
}

func countOutputTokens(text string) int {
	tk, err := getTokenizer()
	if err != nil {
		fmt.Printf("Error loading tokenizer: %v\n", err)
		return 0
	}
	inputSeq := tokenizer.NewInputSequence(text)
	encoding, _ := tk.Encode(tokenizer.NewSingleEncodeInput(inputSeq), false)
	return len(encoding.GetIds())
}

func getPrompt(meanInputTokens, stddevInputTokens, meanOutputTokens, stddevOutputTokens int) (string, int) {
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

	tk, err := getTokenizer()
	if err != nil {
		fmt.Printf("Error loading tokenizer: %v\n", err)
		return "", 0
	}

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

func printMetricsOverview(ttfts, throughputs []float64, inputTokensList, outputTokensList []int) {
	fmt.Println("Performance Metrics")
	fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")

	printTTFTMetrics(ttfts)
	printThroughputMetrics(throughputs)
	printTokenCountMetrics(inputTokensList, "3. Input Token Count")
	printTokenCountMetrics(outputTokensList, "4. Output Token Count")
}

func printRequestSummary(totalRequests, successfulRequests, failedRequests int) {
	fmt.Println("\nRequest Summary")
	fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")

	successRate := float64(successfulRequests) / float64(totalRequests) * 100
	failureRate := float64(failedRequests) / float64(totalRequests) * 100

	fmt.Printf("Total Requests:    %d\n", totalRequests)
	fmt.Printf("Successful:        %d (%.2f%%)\n", successfulRequests, successRate)
	fmt.Printf("Failed:            %d (%.2f%%)\n", failedRequests, failureRate)
}

func printTTFTMetrics(ttfts []float64) {
	if len(ttfts) == 0 {
		fmt.Println("No successful requests.")
		return
	}

	sort.Float64s(ttfts)

	min := ttfts[0]
	p25 := percentile(ttfts, 25)
	median := percentile(ttfts, 50)
	p75 := percentile(ttfts, 75)
	max := ttfts[len(ttfts)-1]
	mean := mean(ttfts)
	stddev := stddev(ttfts, mean)

	labels := []string{"Min", "P25", "Median (P50)", "P75", "Max"}
	values := []string{
		fmt.Sprintf("%.0f ms", min),
		fmt.Sprintf("%.0f ms", p25),
		fmt.Sprintf("%.0f ms", median),
		fmt.Sprintf("%.0f ms", p75),
		fmt.Sprintf("%.0f ms", max),
	}

	printCentered("1. Time To First Token (TTFT) (ms):", labels, values)

	fmt.Printf("\n   Average (Mean): %.0f ms\n", mean)
	fmt.Printf("   Standard Deviation: %.0f ms\n", stddev)
}

func printThroughputMetrics(throughputs []float64) {
	if len(throughputs) == 0 {
		fmt.Println("No successful requests.")
		return
	}

	sort.Float64s(throughputs)

	min := throughputs[0]
	p25 := percentile(throughputs, 25)
	median := percentile(throughputs, 50)
	p75 := percentile(throughputs, 75)
	max := throughputs[len(throughputs)-1]
	mean := mean(throughputs)
	stddev := stddev(throughputs, mean)

	labels := []string{"Min", "P25", "Median (P50)", "P75", "Max"}
	values := []string{
		fmt.Sprintf("%.1f t/s", min),
		fmt.Sprintf("%.1f t/s", p25),
		fmt.Sprintf("%.1f t/s", median),
		fmt.Sprintf("%.1f t/s", p75),
		fmt.Sprintf("%.1f t/s", max),
	}

	printCentered("2. Throughput (tokens/second):", labels, values)

	fmt.Printf("\n   Average (Mean): %.0f t/s\n", mean)
	fmt.Printf("   Standard Deviation: %.0f t/s\n", stddev)
}

func printTokenCountMetrics(tokens []int, metricName string) {
	if len(tokens) == 0 {
		fmt.Println("No successful requests.")
		return
	}

	tokensFloat := make([]float64, len(tokens))
	for i, v := range tokens {
		tokensFloat[i] = float64(v)
	}

	sort.Float64s(tokensFloat)

	min := tokensFloat[0]
	p25 := percentile(tokensFloat, 25)
	median := percentile(tokensFloat, 50)
	p75 := percentile(tokensFloat, 75)
	max := tokensFloat[len(tokensFloat)-1]
	mean := mean(tokensFloat)
	stddev := stddev(tokensFloat, mean)

	labels := []string{"Min", "P25", "Median (P50)", "P75", "Max"}
	values := []string{
		fmt.Sprintf("%.0f", min),
		fmt.Sprintf("%.0f", p25),
		fmt.Sprintf("%.0f", median),
		fmt.Sprintf("%.0f", p75),
		fmt.Sprintf("%.0f", max),
	}

	printCentered(metricName+":", labels, values)

	fmt.Printf("\n   Average (Mean): %.0f\n", mean)
	fmt.Printf("   Standard Deviation: %.0f\n", stddev)
}

func printCentered(title string, labels, values []string) {
	fmt.Println()
	fmt.Println(title)
	fmt.Println("       [───────────────|───────────────|───────────────|───────────────]")

	labelLine := ""
	valueLine := ""

	for i, label := range labels {
		labelLine += alignCenter(label, 15)
		if i < len(labels)-1 {
			labelLine += " "
		}
	}

	for i, value := range values {
		valueLine += alignCenter(value, 15)
		if i < len(values)-1 {
			valueLine += " "
		}
	}

	fmt.Println(labelLine)
	fmt.Println(valueLine)
}

func alignCenter(s string, width int) string {
	if len(s) >= width {
		return s
	}
	leftPad := (width - len(s)) / 2
	rightPad := width - len(s) - leftPad
	return strings.Repeat(" ", leftPad) + s + strings.Repeat(" ", rightPad)
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
