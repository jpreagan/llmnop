package cmd

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/sashabaranov/go-openai"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/mock"
)

type MockTimeProvider struct {
	mock.Mock
	currentTime time.Time
}

func (m *MockTimeProvider) Now() time.Time {
	return m.currentTime
}

func (m *MockTimeProvider) Since(t time.Time) time.Duration {
	return m.currentTime.Sub(t)
}

func (m *MockTimeProvider) Advance(d time.Duration) {
	m.currentTime = m.currentTime.Add(d)
}

func TestBenchmarkTTFT(t *testing.T) {
	tokenizerPath = "../data/tokenizer.json"

	mockTimeProvider := &MockTimeProvider{currentTime: time.Now()}
	firstContentTokenReceived := false

	mockServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/event-stream")

		responses := []openai.ChatCompletionStreamResponse{
			{
				Choices: []openai.ChatCompletionStreamChoice{
					{Delta: openai.ChatCompletionStreamChoiceDelta{Role: "assistant"}},
				},
			},
			{
				Choices: []openai.ChatCompletionStreamChoice{
					{Delta: openai.ChatCompletionStreamChoiceDelta{Content: "You"}},
				},
			},
			{
				Choices: []openai.ChatCompletionStreamChoice{
					{Delta: openai.ChatCompletionStreamChoiceDelta{Content: "'re"}},
				},
			},
			{
				Choices: []openai.ChatCompletionStreamChoice{
					{Delta: openai.ChatCompletionStreamChoiceDelta{Content: " welcome"}},
				},
			},
			{
				Choices: []openai.ChatCompletionStreamChoice{
					{Delta: openai.ChatCompletionStreamChoiceDelta{Content: "."}},
				},
			},
			{
				Choices: []openai.ChatCompletionStreamChoice{
					{Delta: openai.ChatCompletionStreamChoiceDelta{}, FinishReason: openai.FinishReasonStop},
				},
			},
		}

		for _, response := range responses {
			data, _ := json.Marshal(response)
			fmt.Fprintf(w, "data: %s\n\n", data)
			w.(http.Flusher).Flush()
			if !firstContentTokenReceived && response.Choices[0].Delta.Content != "" {
				mockTimeProvider.Advance(66 * time.Millisecond) // Simulate 66ms delay for the first meaningful token
				firstContentTokenReceived = true
			}
		}

		fmt.Fprintf(w, "data: [DONE]\n\n")
	}))
	defer mockServer.Close()

	prompt := "Thank you."
	inputTokens := 3

	result, err := benchmark(mockServer.URL, "test-key", "test-model", prompt, inputTokens, mockTimeProvider)

	assert.NoError(t, err)
	assert.Equal(t, 66*time.Millisecond, result.TTFT) // TTFT should be 66ms after the first content token
	assert.Equal(t, inputTokens, result.InputTokens)
}

func TestBenchmarkThroughput(t *testing.T) {
	tokenizerPath = "../data/tokenizer.json"

	mockTimeProvider := &MockTimeProvider{currentTime: time.Now()}
	startTime := mockTimeProvider.Now()

	mockServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/event-stream")

		responses := []openai.ChatCompletionStreamResponse{
			{
				Choices: []openai.ChatCompletionStreamChoice{
					{Delta: openai.ChatCompletionStreamChoiceDelta{Role: "assistant"}},
				},
			},
			{
				Choices: []openai.ChatCompletionStreamChoice{
					{Delta: openai.ChatCompletionStreamChoiceDelta{Content: "You"}},
				},
			},
			{
				Choices: []openai.ChatCompletionStreamChoice{
					{Delta: openai.ChatCompletionStreamChoiceDelta{Content: "'re"}},
				},
			},
			{
				Choices: []openai.ChatCompletionStreamChoice{
					{Delta: openai.ChatCompletionStreamChoiceDelta{Content: " welcome"}},
				},
			},
			{
				Choices: []openai.ChatCompletionStreamChoice{
					{Delta: openai.ChatCompletionStreamChoiceDelta{Content: "."}},
				},
			},
			{
				Choices: []openai.ChatCompletionStreamChoice{
					{Delta: openai.ChatCompletionStreamChoiceDelta{}, FinishReason: openai.FinishReasonStop},
				},
			},
		}

		for _, response := range responses {
			data, _ := json.Marshal(response)
			fmt.Fprintf(w, "data: %s\n\n", data)
			w.(http.Flusher).Flush()
			mockTimeProvider.Advance(66 * time.Millisecond) // Advance 66ms for each response
		}

		fmt.Fprintf(w, "data: [DONE]\n\n")
	}))
	defer mockServer.Close()

	prompt := "Thank you."
	inputTokens := 3

	result, err := benchmark(mockServer.URL, "test-key", "test-model", prompt, inputTokens, mockTimeProvider)

	assert.NoError(t, err)
	assert.Equal(t, inputTokens, result.InputTokens)
	outputTokens := 5
	assert.Equal(t, outputTokens, result.OutputTokens)

	totalTime := mockTimeProvider.Now().Sub(startTime)
	expectedThroughput := float64(outputTokens) / totalTime.Seconds()
	assert.InDelta(t, expectedThroughput, result.Throughput, 0.01)

	// Additional assertions to verify the total time
	assert.Equal(t, 396*time.Millisecond, totalTime, "Total time should be 396ms (66ms * 6 responses)")
}
