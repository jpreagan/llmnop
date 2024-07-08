package cmd

import (
	"os"

	"github.com/spf13/cobra"
)

// rootCmd represents the base command when called without any subcommands
var rootCmd = &cobra.Command{
	Use:   "llmnop",
	Short: "LLMNOP is a CLI tool for LLMOps",
	Long: `LLMNOP is a command-line tool for LLMOps used to benchmark Large Language Model (LLM)
performance metrics like throughput and latency.`,
}

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main(). It only needs to happen once to the rootCmd.
func Execute() {
	err := rootCmd.Execute()
	if err != nil {
		os.Exit(1)
	}
}
