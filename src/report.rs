use crate::output::BenchmarkSummary;
use crate::style::{ACCENT, content_style, group, seconds};
use ratatui::crossterm::queue;
use ratatui::crossterm::style::{PrintStyledContent, StyledContent};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use std::io::{self, Write};
use std::path::Path;

const LABEL_WIDTH: usize = 12;

type Metric = (&'static str, &'static str, fn(Option<f64>) -> String);

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

fn duration(ms: Option<f64>) -> String {
    ms.map_or_else(|| "—".into(), seconds)
}

fn rate(value: Option<f64>) -> String {
    number(value, 1)
}

fn count(value: Option<f64>) -> String {
    number(value, 0)
}

fn outcome(summary: &BenchmarkSummary<'_>) -> Line<'static> {
    let m = &summary.measurement;
    let mut spans = vec![Span::raw(format!("{} requests", m.started))];
    for (n, what, color) in [
        (m.completed, "succeeded", Color::Green),
        (m.failed, "failed", Color::Red),
        (m.timed_out, "timed out", Color::Red),
        (m.cancelled, "cancelled", Color::Yellow),
        (
            m.completed_at_output_limit,
            "stopped at the output cap",
            Color::Yellow,
        ),
        (m.completed_with_no_text, "returned no text", Color::Yellow),
    ] {
        if n > 0 {
            spans.push(" · ".dim());
            spans.push(n.to_string().fg(color).bold());
            spans.push(format!(" {what}").dim());
        }
    }
    Line::from(spans)
}

pub fn report(summary: &BenchmarkSummary<'_>, directory: &Path) -> Text<'static> {
    let args = summary.configuration;
    let m = &summary.measurement;
    let sections: [(&str, &[Metric]); 3] = [
        (
            "Latency",
            &[
                ("Time to first token", "ttft_ms", duration),
                ("Time to first content", "ttfo_ms", duration),
                ("Request latency", "request_latency_ms", duration),
            ],
        ),
        (
            "Generation",
            &[(
                "Throughput per request (tokens/s)",
                "generation_tokens_per_second",
                rate,
            )],
        ),
        (
            "Tokens per request",
            &[
                ("Input", "input_tokens", count),
                ("Reasoning", "reasoning_tokens", count),
                ("Content", "content_tokens", count),
                ("Generated", "generated_tokens", count),
            ],
        ),
    ];
    let mut name_width = 0;
    let mut value_width = 9;
    for (name, key, format) in sections.iter().flat_map(|(_, rows)| rows.iter()) {
        name_width = name_width.max(name.chars().count());
        let s = &summary.metrics[key];
        for value in [s.mean, s.p50, s.p95, s.p99] {
            value_width = value_width.max(format(value).chars().count());
        }
    }
    let width = 2 + name_width + 4 * (value_width + 1);

    let mut head = vec![
        "llmnop".fg(ACCENT).bold(),
        "  ".into(),
        args.model.clone().unwrap_or_default().bold(),
        "  ".into(),
        format!("{} · concurrency {}", args.api, args.concurrency).dim(),
    ];
    let used: usize = head.iter().map(Span::width).sum();
    if let Some(d) = summary.measurement_duration_ms {
        let d = seconds(d);
        let gap = width.saturating_sub(used + d.chars().count()).max(2);
        head.push(" ".repeat(gap).into());
        head.push(d.bold());
    }
    let mut lines = vec![Line::from(head), outcome(summary)];
    if summary.termination == "interrupted" {
        lines.push(Line::from(
            format!(
                "interrupted after {} of {} requests started",
                m.started, args.requests
            )
            .yellow(),
        ));
    }
    if !summary.errors.is_empty() {
        let n: usize = summary.errors.values().sum();
        let categories: Vec<_> = summary
            .errors
            .iter()
            .map(|(category, n)| format!("{category} ×{n}"))
            .collect();
        let noun = if n == 1 { "failure" } else { "failures" };
        lines.push(Line::from(vec![
            format!("{n} {noun}: ").dim(),
            categories.join(" · ").red(),
        ]));
    }
    if summary.warmup.unsuccessful() > 0 {
        lines.push(Line::from(
            format!(
                "{} of {} warmup requests did not complete",
                summary.warmup.unsuccessful(),
                summary.warmup.started
            )
            .yellow(),
        ));
    }
    lines.push(Line::default());
    lines.push(Line::from(
        format!(
            "  {:<name_width$} {:>value_width$} {:>value_width$} {:>value_width$} {:>value_width$}",
            "", "mean", "p50", "p95", "p99"
        )
        .dim(),
    ));
    for (i, (title, rows)) in sections.into_iter().enumerate() {
        if i > 0 {
            lines.push(Line::default());
        }
        lines.push(Line::from(title.fg(ACCENT).bold()));
        for (name, key, format) in rows {
            let s = &summary.metrics[key];
            let cell = |v: Option<f64>| format!(" {:>value_width$}", format(v));
            let style = if s.count == 0 {
                Style::new().dim()
            } else {
                Style::new()
            };
            lines.push(Line::from(vec![
                format!("  {name:<name_width$}").into(),
                Span::styled([s.mean, s.p50, s.p95, s.p99].map(cell).concat(), style),
            ]));
        }
    }
    lines.push(Line::default());
    let label = |s: &str| {
        format!("{s:<width$}", width = LABEL_WIDTH)
            .fg(ACCENT)
            .bold()
    };
    let requests_per_second = summary.completed_requests_per_second;
    let decimals = if requests_per_second.is_some_and(|r| r < 1.0) {
        3
    } else {
        2
    };
    lines.push(Line::from(vec![
        label("Throughput"),
        number(requests_per_second, decimals).bold(),
        " requests/s".dim(),
        " · ".dim(),
        number(summary.completed_generated_tokens_per_second, 1).bold(),
        " generated tokens/s".dim(),
    ]));
    lines.push(Line::from(vec![
        label("Results"),
        directory.display().to_string().into(),
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
    use crate::args::Args;
    use clap::Parser;

    #[test]
    fn metric_values_stay_aligned_under_their_headers() {
        let args = Args::parse_from(["llmnop"]);
        let mut summary = BenchmarkSummary::new(&args, "test".into(), &[], false);
        for (key, value) in [
            ("ttft_ms", 12.3),
            ("request_latency_ms", 1_000_000.0),
            ("input_tokens", 10_000_000.0),
        ] {
            let stats = summary.metrics.get_mut(key).unwrap();
            stats.count = 1;
            stats.mean = Some(value);
            stats.p50 = Some(value);
            stats.p95 = Some(value);
            stats.p99 = Some(value);
        }
        let mut output = Vec::new();
        write(&mut output, &report(&summary, Path::new("results")), false).unwrap();
        let output = String::from_utf8(output).unwrap();
        let header: Vec<char> = output
            .lines()
            .find(|line| line.trim_start().starts_with("mean"))
            .unwrap()
            .chars()
            .collect();
        let ends: Vec<usize> = ["mean", "p50", "p95", "p99"]
            .iter()
            .map(|name| {
                let name: Vec<char> = name.chars().collect();
                header
                    .windows(name.len())
                    .position(|w| w == name.as_slice())
                    .unwrap()
                    + name.len()
            })
            .collect();
        for (label, expected) in [
            ("Request latency", "16m 40s"),
            ("Time to first token", "0.01s"),
            ("Time to first content", "—"),
            ("Input", "10,000,000"),
        ] {
            let line: Vec<char> = output
                .lines()
                .find(|line| line.trim_start().starts_with(label))
                .unwrap()
                .chars()
                .collect();
            let expected: Vec<char> = expected.chars().collect();
            for &end in &ends {
                let start = end - expected.len();
                assert_eq!(&line[start..end], expected.as_slice(), "{label}");
                assert_eq!(line[start - 1], ' ', "{label}");
            }
        }
    }
}
