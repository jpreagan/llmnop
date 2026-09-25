use crate::args::Args;
use crate::output::BenchmarkSummary;
use crate::style::{ACCENT, content_style, group, seconds};
use ratatui::crossterm::queue;
use ratatui::crossterm::style::{PrintStyledContent, StyledContent};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use std::io::{self, Write};
use std::path::Path;

const LABEL_WIDTH: usize = 12;

type Metric = (&'static str, &'static str, usize);

fn endpoint(args: &Args) -> Vec<Span<'static>> {
    vec![
        args.model.clone().unwrap_or_default().bold(),
        " · ".dim(),
        args.api.to_string().fg(ACCENT),
        " · ".dim(),
        args.url.clone().unwrap_or_default().dim(),
    ]
}

fn workload(args: &Args) -> String {
    let mut line = format!("{} input tokens", args.input_tokens);
    if args.input_tokens_stddev > 0 {
        line.push_str(&format!(" ±{}", args.input_tokens_stddev));
    }
    match args.output_cap {
        Some(cap) if args.output_cap_stddev > 0 => {
            line.push_str(&format!(" · output cap {cap} ±{}", args.output_cap_stddev));
        }
        Some(cap) => line.push_str(&format!(" · output cap {cap}")),
        None => line.push_str(" · no output cap"),
    }
    line.push_str(&format!(
        " · {} requests · concurrency {}",
        args.requests, args.concurrency
    ));
    if args.warmup > 0 {
        line.push_str(&format!(" · warmup {}", args.warmup));
    }
    line.push_str(&format!(" · timeout {}s", args.request_timeout));
    line
}

fn number(value: Option<f64>, decimals: usize) -> String {
    let Some(v) = value else {
        return "—".into();
    };
    let sign = if v < 0.0 { "-" } else { "" };
    let fixed = format!("{:.decimals$}", v.abs());
    let (whole, fraction) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let whole = group(whole);
    if fraction.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{fraction}")
    }
}

pub fn report(summary: &BenchmarkSummary<'_>, directory: &Path) -> Text<'static> {
    let args = summary.configuration;
    let m = &summary.measurement;
    let mut head = vec![
        format!("llmnop {}", summary.llmnop_version)
            .bold()
            .fg(ACCENT),
        " · ".dim(),
    ];
    head.extend(endpoint(args));
    let mut lines = vec![
        Line::from(head),
        Line::from(
            format!(
                "{} · tokenizer {}",
                workload(args),
                args.tokenizer.clone().unwrap_or_default()
            )
            .dim(),
        ),
        Line::default(),
    ];
    let label = |s: &str| format!("{s:<width$}", width = LABEL_WIDTH).bold();
    let indent = || Span::raw(" ".repeat(LABEL_WIDTH));
    let outcome_style = if m.completed == m.started && m.started > 0 {
        Style::new().green().bold()
    } else {
        Style::new().yellow().bold()
    };
    let mut outcome = vec![
        label("Outcome"),
        Span::styled(
            format!("{} of {} requests completed", m.completed, m.started),
            outcome_style,
        ),
    ];
    if let Some(d) = summary.measurement_duration_ms {
        outcome.push(format!(" in {}", seconds(d)).into());
    }
    for (n, what, color) in [
        (m.failed, "failed", Color::Red),
        (m.timed_out, "timed out", Color::Yellow),
        (m.cancelled, "cancelled", Color::Yellow),
    ] {
        if n > 0 {
            outcome.push(" · ".dim());
            outcome.push(format!("{n} {what}").fg(color).bold());
        }
    }
    lines.push(Line::from(outcome));
    if summary.termination == "interrupted" {
        lines.push(Line::from(vec![
            indent(),
            format!(
                "interrupted after {} of {} requests started",
                m.started, args.requests
            )
            .yellow(),
        ]));
    }
    lines.push(Line::from(vec![
        indent(),
        format!(
            "{} stopped at the output cap · {} returned no text",
            m.completed_at_output_limit, m.completed_with_no_text
        )
        .dim(),
    ]));
    if m.failed > 0 {
        let categories: Vec<_> = summary
            .errors
            .iter()
            .map(|(category, n)| format!("{category} ×{n}"))
            .collect();
        lines.push(Line::from(vec![indent(), categories.join(" · ").red()]));
    }
    if summary.warmup.unsuccessful() > 0 {
        lines.push(Line::from(vec![
            indent(),
            format!(
                "{} of {} warmup requests did not complete",
                summary.warmup.unsuccessful(),
                summary.warmup.started
            )
            .yellow(),
        ]));
    }
    lines.push(Line::default());
    let sections: [(&str, &[Metric]); 3] = [
        (
            "Latency",
            &[
                ("Time to first token (ms)", "ttft_ms", 1),
                ("Time to first content (ms)", "ttfo_ms", 1),
                ("Request latency (ms)", "request_latency_ms", 1),
            ],
        ),
        (
            "Generation",
            &[(
                "Throughput per request (tokens/s)",
                "generation_tokens_per_second",
                1,
            )],
        ),
        (
            "Tokens per request",
            &[
                ("Input", "input_tokens", 0),
                ("Reasoning", "reasoning_tokens", 0),
                ("Content", "content_tokens", 0),
                ("Generated", "generated_tokens", 0),
            ],
        ),
    ];
    let mut name_width = 0;
    let mut samples_width = 7;
    let mut value_width = 9;
    for (name, key, decimals) in sections.iter().flat_map(|(_, rows)| rows.iter()) {
        name_width = name_width.max(name.chars().count());
        let s = &summary.metrics[key];
        samples_width = samples_width.max(s.count.to_string().len());
        for value in [s.mean, s.p50, s.p95, s.p99] {
            value_width = value_width.max(number(value, *decimals).chars().count());
        }
    }
    lines.push(Line::from(
        format!(
            "  {:<name_width$} {:>samples_width$} {:>value_width$} {:>value_width$} {:>value_width$} {:>value_width$}",
            "", "samples", "mean", "p50", "p95", "p99"
        )
        .dim(),
    ));
    for (title, rows) in sections {
        lines.push(Line::from(title.bold()));
        for (name, key, decimals) in rows {
            let s = &summary.metrics[key];
            let cell = |v: Option<f64>| format!(" {:>value_width$}", number(v, *decimals));
            let style = if s.count == 0 {
                Style::new().dim()
            } else {
                Style::new()
            };
            lines.push(Line::from(vec![
                format!("  {name:<name_width$}").into(),
                format!(" {:>samples_width$}", s.count).dim(),
                Span::styled([s.mean, s.p50, s.p95, s.p99].map(cell).concat(), style),
            ]));
        }
    }
    lines.push(Line::default());
    lines.push(Line::from(vec![
        label("Throughput"),
        format!(
            "{} requests/s · {} generated tokens/s",
            number(summary.completed_requests_per_second, 2),
            number(summary.completed_generated_tokens_per_second, 1)
        )
        .into(),
        summary
            .measurement_duration_ms
            .map(|d| format!(" over {}", seconds(d)))
            .unwrap_or_default()
            .dim(),
    ]));
    lines.push(Line::default());
    lines.push(Line::from(vec![
        label("Results"),
        directory.display().to_string().into(),
        "  (summary.json · requests.jsonl)".dim(),
    ]));
    Text::from(lines)
}

pub fn write(out: &mut impl Write, text: &Text<'_>, color: bool) -> io::Result<()> {
    if color {
        for line in &text.lines {
            for span in &line.spans {
                queue!(
                    out,
                    PrintStyledContent(StyledContent::new(
                        content_style(span.style),
                        span.content.as_ref()
                    ))
                )?;
            }
            out.write_all(b"\n")?;
        }
    } else {
        writeln!(out, "{text}")?;
    }
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn metric_columns_stay_separate_and_aligned_for_large_values() {
        let args = Args::parse_from(["llmnop"]);
        let mut summary = BenchmarkSummary::new(&args, "test".into(), &[], false);
        for (key, count, value) in [
            ("ttft_ms", 1, 12.3),
            ("request_latency_ms", 1, 1_000_000.0),
            ("input_tokens", 123_456_789, 10_000_000.0),
        ] {
            let stats = summary.metrics.get_mut(key).unwrap();
            stats.count = count;
            stats.mean = Some(value);
            stats.p50 = Some(value);
            stats.p95 = Some(value);
            stats.p99 = Some(value);
        }
        let mut output = Vec::new();
        write(&mut output, &report(&summary, Path::new("results")), false).unwrap();
        let output = String::from_utf8(output).unwrap();
        let column_ends = |line: &str| {
            let chars: Vec<_> = line.chars().chain([' ']).collect();
            chars
                .windows(2)
                .enumerate()
                .filter_map(|(i, pair)| {
                    (!pair[0].is_whitespace() && pair[1].is_whitespace()).then_some(i + 1)
                })
                .rev()
                .take(5)
                .collect::<Vec<_>>()
        };
        let header = output
            .lines()
            .find(|line| line.contains("samples"))
            .unwrap();
        for (label, expected) in [
            (
                "Request latency (ms)",
                [
                    "1",
                    "1,000,000.0",
                    "1,000,000.0",
                    "1,000,000.0",
                    "1,000,000.0",
                ],
            ),
            (
                "Time to first token (ms)",
                ["1", "12.3", "12.3", "12.3", "12.3"],
            ),
            ("Time to first content (ms)", ["0", "—", "—", "—", "—"]),
            (
                "Input",
                [
                    "123456789",
                    "10,000,000",
                    "10,000,000",
                    "10,000,000",
                    "10,000,000",
                ],
            ),
        ] {
            let line = output
                .lines()
                .find(|line| line.trim_start().starts_with(label))
                .unwrap();
            let cells: Vec<_> = line
                .trim_start()
                .strip_prefix(label)
                .unwrap()
                .split_whitespace()
                .collect();
            assert_eq!(cells, expected, "{line}");
            assert_eq!(column_ends(line), column_ends(header), "{line}");
        }
    }
}
