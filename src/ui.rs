mod terminal;

use crate::args::Args;
use crate::benchmark::{Delta, Phase, RequestRecord, Status, seconds_per_token};
use crate::output::{Counts, MetricStats};
use crate::report;
use crate::style::{ACCENT, group, seconds};
use crate::tokens;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Cell, LineGauge, Padding, Row, Sparkline, Table, Widget,
};
use std::collections::{BTreeMap, VecDeque};
use std::io::{self, BufWriter, IsTerminal, Stderr};
use std::time::{Duration, Instant};
use terminal::Inline;
use tokenizers::Tokenizer;
use tokio::sync::watch;
use tokio::time::{Interval, MissedTickBehavior};

const REFRESH: Duration = Duration::from_millis(100);
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
// Viewport rows besides one per visible in-flight request: the border, the endpoint,
// workload, progress, tallies, statistics, and sparkline lines, three spacers, and
// the in-flight table header.
const FIXED_ROWS: u16 = 12;
const MAX_FLIGHT_ROWS: u16 = 8;
const COLUMN_GAP: u16 = 2;
const COLUMNS: [(&str, u16, Alignment); 8] = [
    ("request", 8, Alignment::Left),
    ("elapsed", 8, Alignment::Right),
    ("ttft", 7, Alignment::Right),
    ("reasoning", 9, Alignment::Right),
    ("answer", 8, Alignment::Right),
    ("tok/s", 6, Alignment::Right),
    ("last delta", 13, Alignment::Right),
    ("of cap", 15, Alignment::Left),
];

/// Paces redraws. After a stall the next frame is drawn on time rather than
/// every missed one in a burst.
pub fn frames() -> Interval {
    let mut interval = tokio::time::interval(REFRESH);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    interval
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Tokenizer,
    Prompts,
    Run(Phase),
}

struct Live {
    started: Instant,
    first: Option<Instant>,
    last: Option<Instant>,
    // Streamed text waiting to be counted at the next frame.
    pending_content: String,
    pending_reasoning: String,
    content: u64,
    reasoning: u64,
    cap: Option<u32>,
}

impl Live {
    fn new(cap: Option<u32>) -> Self {
        Self {
            started: Instant::now(),
            first: None,
            last: None,
            pending_content: String::new(),
            pending_reasoning: String::new(),
            content: 0,
            reasoning: 0,
            cap,
        }
    }
}

struct State {
    endpoint: Line<'static>,
    workload: Line<'static>,
    tokenizer_name: String,
    stage: Stage,
    stage_started: Instant,
    total: usize,
    cancel: watch::Receiver<bool>,
    live: BTreeMap<u32, Live>,
    tally: Counts,
    ttft: Vec<f64>,
    latency: Vec<f64>,
    rate: Vec<f64>,
    buckets: VecDeque<u64>,
    bucket_at: Instant,
}

pub struct Ui {
    terminal: Option<Inline<BufWriter<Stderr>>>,
    state: State,
    trails: Vec<Line<'static>>,
}

impl Ui {
    pub fn new(args: &Args, cancel: watch::Receiver<bool>) -> Self {
        let state = State {
            endpoint: Line::from(report::endpoint(args)),
            workload: Line::from(report::workload(args)).dim(),
            tokenizer_name: args.tokenizer.clone().unwrap_or_default(),
            stage: Stage::Tokenizer,
            stage_started: Instant::now(),
            total: 0,
            cancel,
            live: BTreeMap::new(),
            tally: Counts::default(),
            ttft: Vec::new(),
            latency: Vec::new(),
            rate: Vec::new(),
            buckets: VecDeque::from([0]),
            bucket_at: Instant::now(),
        };
        let flight_rows = args.concurrency.min(MAX_FLIGHT_ROWS.into()) as u16;
        let supports_dashboard = std::env::var_os("TERM")
            .map(|term| !term.is_empty() && term != "dumb")
            .unwrap_or(cfg!(windows));
        let terminal = (io::stderr().is_terminal() && supports_dashboard)
            .then(|| Inline::new(BufWriter::new(io::stderr()), FIXED_ROWS + flight_rows))
            .and_then(Result::ok);
        Self {
            terminal,
            state,
            trails: Vec::new(),
        }
    }

    pub fn interactive(&self) -> bool {
        self.terminal.is_some()
    }

    /// Runs a blocking task while showing progress. Returns `None` when the run is
    /// interrupted first.
    pub async fn attend<T: Send + 'static>(
        &mut self,
        stage: Stage,
        total: usize,
        task: impl FnOnce() -> T + Send + 'static,
    ) -> Option<T> {
        self.begin(stage, total);
        let mut task = tokio::task::spawn_blocking(task);
        let mut tick = frames();
        let mut cancel = self.state.cancel.clone();
        loop {
            tokio::select! {
                result = &mut task => return Some(result.expect("preparation task panicked")),
                _ = cancel.wait_for(|cancelled| *cancelled) => return None,
                _ = tick.tick() => self.draw(),
            }
        }
    }

    pub fn begin(&mut self, stage: Stage, total: usize) {
        let state = &mut self.state;
        state.stage = stage;
        state.stage_started = Instant::now();
        state.total = total;
        state.live.clear();
        state.tally = Counts::default();
        state.ttft.clear();
        state.latency.clear();
        state.rate.clear();
        if self.terminal.is_none() {
            eprintln!("{}", state.stage_label());
        }
        self.draw();
    }

    pub fn started(&mut self, id: u32, cap: Option<u32>) {
        self.state.live.insert(id, Live::new(cap));
    }

    pub fn delta(&mut self, delta: Delta) {
        if let Some(live) = self.state.live.get_mut(&delta.id) {
            live.pending_content.push_str(&delta.content);
            live.pending_reasoning.push_str(&delta.reasoning);
            live.first.get_or_insert(delta.at);
            live.last = Some(delta.at);
        }
    }

    /// Counts the text streamed since the last frame. Tokenizing once per frame
    /// rather than per delta keeps the event loop free while many requests stream.
    pub fn tokenize(&mut self, tokenizer: &Tokenizer) {
        let state = &mut self.state;
        state.advance(Instant::now());
        let mut arrived = 0;
        for live in state.live.values_mut() {
            let content = drain(&mut live.pending_content, tokenizer);
            let reasoning = drain(&mut live.pending_reasoning, tokenizer);
            live.content += content;
            live.reasoning += reasoning;
            arrived += content + reasoning;
        }
        *state.buckets.back_mut().unwrap() += arrived;
    }

    pub fn finished(&mut self, record: &RequestRecord) {
        let state = &mut self.state;
        if let Some(live) = state.live.remove(&record.request_id) {
            if let Some(generated) = record.metrics.generated_tokens {
                // Final counts include text whose deltas have not reached the UI yet.
                let remaining = generated.saturating_sub(live.content + live.reasoning);
                state.advance(Instant::now());
                *state.buckets.back_mut().unwrap() += remaining;
            }
        }
        state.tally.add(record);
        if record.status == Status::Completed {
            let m = &record.metrics;
            state.ttft.extend(m.ttft_ms);
            state.latency.extend(m.request_latency_ms);
            state.rate.extend(m.generation_tokens_per_second);
        }
        let line = trail(record);
        if self.interactive() {
            self.trails.push(line);
        } else {
            eprintln!("{line}");
        }
    }

    pub fn draw(&mut self) {
        self.flush_trails();
        let Some(terminal) = self.terminal.as_mut() else {
            return;
        };
        self.state.advance(Instant::now());
        let state = &self.state;
        let _ = terminal.draw(|frame| state.render(frame.area(), frame.buffer_mut()));
    }

    fn flush_trails(&mut self) {
        if let Some(terminal) = self.terminal.as_mut() {
            let _ = terminal.insert_before(&self.trails);
        }
        self.trails.clear();
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        // A phase may finish before the next frame, including when interrupted.
        self.flush_trails();
    }
}

fn drain(pending: &mut String, tokenizer: &Tokenizer) -> u64 {
    if pending.is_empty() {
        return 0;
    }
    let count = tokens::count(tokenizer, pending).unwrap_or(0);
    pending.clear();
    count
}

fn trail(record: &RequestRecord) -> Line<'static> {
    let m = &record.metrics;
    let id = format!("#{}", record.request_id).bold();
    let phase = match record.phase {
        Phase::Warmup => "warmup ",
        Phase::Measurement => "",
    }
    .dim();
    let elapsed = seconds(record.elapsed_ms);
    let tokens = m
        .generated_tokens
        .map(|n| format!("{} tok", group(n)))
        .unwrap_or_default();
    if record.status == Status::Completed {
        let mut spans = vec!["  ✓ ".green(), phase, id, format!("  {elapsed}").into()];
        if let Some(ttft) = m.ttft_ms {
            spans.push(format!(" · ttft {}", seconds(ttft)).dim());
        }
        spans.push(format!(" · {tokens}").dim());
        if let Some(r) = m.reasoning_tokens.filter(|r| *r > 0) {
            spans.push(format!(" ({} reasoning)", group(r)).dim());
        }
        if let Some(rate) = m.generation_tokens_per_second {
            spans.push(format!(" · {rate:.1} tok/s").dim());
        }
        if record.stopped_at_output_cap() {
            spans.push(" · output cap".yellow());
        }
        return Line::from(spans);
    }
    let (mark, what, style) = match record.status {
        Status::TimedOut => ("✗", "timed out", Style::new().yellow()),
        Status::Cancelled => ("–", "cancelled", Style::new().yellow()),
        _ => ("✗", "failed", Style::new().red()),
    };
    let detail = match &record.error {
        Some(e) if record.status == Status::Failed => {
            format!(" · {}: {}", e.category, e.message.replace('\n', " "))
        }
        _ if tokens.is_empty() => String::new(),
        _ => format!(" · {tokens}"),
    };
    Line::from(vec![
        Span::styled(format!("  {mark} "), style),
        phase,
        id,
        Span::styled(format!("  {what} after {elapsed}"), style),
        detail.dim(),
    ])
}

impl State {
    fn advance(&mut self, now: Instant) {
        while now.duration_since(self.bucket_at) >= Duration::from_secs(1) {
            self.bucket_at += Duration::from_secs(1);
            self.buckets.push_back(0);
            if self.buckets.len() > 300 {
                self.buckets.pop_front();
            }
        }
    }

    // Records reach the dashboard as requests finish, so every counted start has ended.
    fn finished(&self) -> usize {
        self.tally.started
    }

    fn stage_label(&self) -> String {
        match self.stage {
            Stage::Tokenizer => format!("Loading tokenizer {}", self.tokenizer_name),
            Stage::Prompts => format!("Generating {} prompts", self.total),
            Stage::Run(Phase::Warmup) => format!("Warmup: {} requests", self.total),
            Stage::Run(Phase::Measurement) => format!("Measuring: {} requests", self.total),
        }
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().dim())
            .title_top(Line::from(" llmnop ").bold().fg(ACCENT))
            .padding(Padding::horizontal(1));
        let inner = block.inner(area);
        block.render(area, buf);
        let [
            head,
            workload,
            _,
            progress,
            tallies,
            _,
            flight,
            _,
            stats,
            spark,
        ] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);
        (&self.endpoint).render(head, buf);
        (&self.workload).render(workload, buf);
        self.render_progress(progress, buf);
        if matches!(self.stage, Stage::Run(_)) {
            self.render_tallies(tallies, buf);
            self.render_flight(flight, buf);
            self.render_stats(stats, buf);
            self.render_spark(spark, buf);
        }
    }

    fn render_progress(&self, area: Rect, buf: &mut Buffer) {
        let elapsed = self.stage_started.elapsed();
        let spinner = SPINNER[(elapsed.as_millis() / 100) as usize % SPINNER.len()];
        let Stage::Run(phase) = self.stage else {
            return Line::from(vec![
                format!("{spinner} ").fg(ACCENT),
                self.stage_label().bold(),
                format!("  {}", clock(elapsed)).dim(),
            ])
            .render(area, buf);
        };
        let cancelling = *self.cancel.borrow();
        let label = match (cancelling, phase) {
            (true, _) => "Cancelling".yellow().bold(),
            (false, Phase::Warmup) => "Warmup".bold(),
            (false, Phase::Measurement) => "Measuring".bold(),
        };
        let done = self.finished();
        let right = Line::from(format!(
            "{}/{} · {} elapsed{}",
            done,
            self.total,
            clock(elapsed),
            self.eta()
                .map(|eta| format!(" · ~{} left", clock(eta)))
                .unwrap_or_default()
        ))
        .right_aligned()
        .dim();
        let [left, gauge, right_area] = Layout::horizontal([
            Constraint::Length(13),
            Constraint::Fill(1),
            Constraint::Length(right.width() as u16 + 1),
        ])
        .areas(area);
        Line::from(vec![format!("{spinner} ").fg(ACCENT), label]).render(left, buf);
        LineGauge::default()
            .ratio(if self.total == 0 {
                0.0
            } else {
                (done as f64 / self.total as f64).min(1.0)
            })
            .label("")
            .filled_style(Style::new().fg(if cancelling { Color::Yellow } else { ACCENT }))
            .unfilled_style(Style::new().dim())
            .render(gauge, buf);
        right.render(right_area, buf);
    }

    fn eta(&self) -> Option<Duration> {
        if self.latency.is_empty() || *self.cancel.borrow() {
            return None;
        }
        let mean = Duration::from_secs_f64(
            self.latency.iter().sum::<f64>() / self.latency.len() as f64 / 1000.0,
        );
        let slots = self.live.len().max(1) as u32;
        let queued = self.total.saturating_sub(self.finished() + self.live.len()) as u32;
        let in_flight = self
            .live
            .values()
            .map(|l| mean.saturating_sub(l.started.elapsed()))
            .max()
            .unwrap_or_default();
        Some(in_flight + mean * queued / slots)
    }

    fn render_tallies(&self, area: Rect, buf: &mut Buffer) {
        let count = |n: usize, label: &str, color: Color| -> Vec<Span<'static>> {
            let style = if n > 0 {
                Style::new().fg(color).bold()
            } else {
                Style::new().dim()
            };
            vec![
                Span::styled(n.to_string(), style),
                format!(" {label}   ").dim(),
            ]
        };
        let tally = &self.tally;
        let mut spans = count(tally.completed, "completed", Color::Green);
        spans.extend(count(tally.failed, "failed", Color::Red));
        spans.extend(count(tally.timed_out, "timed out", Color::Yellow));
        if tally.cancelled > 0 {
            spans.extend(count(tally.cancelled, "cancelled", Color::Yellow));
        }
        spans.extend(count(self.live.len(), "in flight", ACCENT));
        if tally.completed_at_output_limit > 0 {
            spans.extend(count(
                tally.completed_at_output_limit,
                "at output cap",
                Color::Yellow,
            ));
        }
        Line::from(spans).render(area, buf);
    }

    fn render_flight(&self, mut area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let capacity = area.height.saturating_sub(1) as usize;
        let visible = capacity.saturating_sub(usize::from(self.live.len() > capacity));
        let hidden = self.live.len().saturating_sub(visible);
        if hidden > 0 {
            let footer = Rect::new(area.x, area.bottom() - 1, area.width, 1);
            Line::from(format!("… {hidden} more in flight").dim()).render(footer, buf);
            area.height -= 1;
            if area.is_empty() {
                return;
            }
        }
        // Keep complete columns, dropping details from the right as space runs out.
        let mut shown = 1;
        let mut used = COLUMNS[0].1;
        for (_, width, _) in &COLUMNS[1..] {
            used += COLUMN_GAP + width;
            if used > area.width {
                break;
            }
            shown += 1;
        }
        let columns = &COLUMNS[..shown];
        let row = |cells: [Line<'static>; 8]| {
            Row::new(
                cells
                    .into_iter()
                    .zip(columns)
                    .map(|(cell, (_, _, alignment))| Cell::from(cell.alignment(*alignment))),
            )
        };
        let header = row(COLUMNS.map(|(name, _, _)| Line::from(name))).style(Style::new().dim());
        let now = Instant::now();
        let rows = self.live.iter().take(visible).map(|(id, live)| {
            let elapsed = now.duration_since(live.started);
            let generated = live.content + live.reasoning;
            let rate = live
                .first
                .zip(live.last)
                .and_then(|(a, b)| seconds_per_token(b.duration_since(a), generated))
                .map(|s| 1.0 / s);
            let ttft = live
                .first
                .map(|t| seconds_of(t.duration_since(live.started)));
            let since = live.last.map(|t| now.duration_since(t));
            let last = match since {
                None if elapsed.as_secs() >= 10 => "no output yet".yellow(),
                None => "no output yet".dim(),
                Some(gap) if gap.as_secs() >= 10 => {
                    format!("stalled {}", seconds_of(gap)).red().bold()
                }
                Some(gap) if gap.as_secs() >= 3 => seconds_of(gap).yellow(),
                Some(gap) => seconds_of(gap).dim(),
            };
            let progress = live
                .cap
                .map(|cap| {
                    let ratio = (generated as f64 / cap as f64).min(1.0);
                    let filled = (ratio * 10.0).round() as usize;
                    Line::from(vec![
                        "▰".repeat(filled).fg(ACCENT),
                        "▱".repeat(10 - filled).dim(),
                        format!(" {:>3}%", (ratio * 100.0) as u32).dim(),
                    ])
                })
                .unwrap_or_default();
            row([
                Line::from(format!("#{id}").bold()),
                Line::from(seconds_of(elapsed)),
                Line::from(ttft.unwrap_or_else(|| "…".into())),
                Line::from(format!("~{}", group(live.reasoning))),
                Line::from(format!("~{}", group(live.content))),
                Line::from(
                    rate.map(|r| format!("{r:.1}"))
                        .unwrap_or_else(|| "…".into()),
                ),
                Line::from(last),
                progress,
            ])
        });
        let widths = columns
            .iter()
            .map(|(_, width, _)| Constraint::Length((*width).min(area.width)));
        Table::new(rows, widths)
            .flex(Flex::Start)
            .header(header)
            .column_spacing(COLUMN_GAP)
            .render(area, buf);
    }

    fn render_stats(&self, area: Rect, buf: &mut Buffer) {
        let line = if self.latency.is_empty() {
            Line::from("no completed requests yet".dim())
        } else {
            let pair = |values: &[f64], f: fn(f64) -> String| {
                let stats = MetricStats::new(values.to_vec());
                match stats.p50.zip(stats.p95) {
                    Some((p50, p95)) => format!("{} · p95 {}", f(p50), f(p95)),
                    None => "—".into(),
                }
            };
            Line::from(vec![
                "completed  ".dim(),
                "ttft ".dim(),
                pair(&self.ttft, seconds).into(),
                "   latency ".dim(),
                pair(&self.latency, seconds).into(),
                "   rate ".dim(),
                pair(&self.rate, |r| format!("{r:.1}")).into(),
                " tok/s".dim(),
            ])
        };
        line.render(area, buf);
    }

    fn render_spark(&self, area: Rect, buf: &mut Buffer) {
        let now = self.buckets.iter().rev().nth(1).copied().unwrap_or(0);
        let label = Line::from(format!("{} tok/s", group(now)))
            .right_aligned()
            .dim();
        let [graph, text] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(label.width() as u16 + 2),
        ])
        .areas(area);
        let width = graph.width as usize;
        let skip = self.buckets.len().saturating_sub(width);
        Sparkline::default()
            .data(self.buckets.iter().skip(skip))
            .style(Style::new().fg(ACCENT))
            .render(graph, buf);
        label.render(text, buf);
    }
}

fn clock(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

fn seconds_of(d: Duration) -> String {
    seconds(d.as_secs_f64() * 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::Metrics;

    fn state(requests: u32) -> State {
        let now = Instant::now();
        State {
            endpoint: Line::from("test"),
            workload: Line::default(),
            tokenizer_name: String::new(),
            stage: Stage::Run(Phase::Measurement),
            stage_started: now,
            total: requests as usize,
            cancel: watch::channel(false).1,
            live: (0..requests).map(|id| (id, Live::new(Some(32)))).collect(),
            tally: Counts::default(),
            ttft: Vec::new(),
            latency: Vec::new(),
            rate: Vec::new(),
            buckets: VecDeque::from([0]),
            bucket_at: now,
        }
    }

    fn lines(buf: &Buffer) -> Vec<String> {
        (buf.area.top()..buf.area.bottom())
            .map(|y| {
                (buf.area.left()..buf.area.right())
                    .map(|x| buf[(x, y)].symbol())
                    .collect()
            })
            .collect()
    }

    fn live_ui() -> Ui {
        Ui {
            terminal: None,
            state: state(1),
            trails: Vec::new(),
        }
    }

    fn record(status: Status, generated_tokens: Option<u64>) -> RequestRecord {
        let now = Instant::now();
        RequestRecord {
            request_id: 0,
            phase: Phase::Measurement,
            start_time_unix_ns: 0,
            end_time_unix_ns: 0,
            elapsed_ms: 0.0,
            status,
            http_status: Some(200),
            finish_reason: None,
            input_target_tokens: 1,
            output_cap: Some(32),
            metrics: Metrics {
                generated_tokens,
                ..Metrics::default()
            },
            reasoning_kinds: Vec::new(),
            provider_usage: None,
            error: None,
            start: now,
            end: now,
        }
    }

    fn delta(content: &str, reasoning: &str) -> Delta {
        Delta {
            id: 0,
            content: content.into(),
            reasoning: reasoning.into(),
            at: Instant::now(),
        }
    }

    #[test]
    fn finished_requests_count_pending_and_queued_text_once() {
        for status in [
            Status::Completed,
            Status::Failed,
            Status::TimedOut,
            Status::Cancelled,
        ] {
            for queued in [false, true] {
                let mut ui = live_ui();
                ui.delta(delta("", "a"));
                if !queued {
                    ui.delta(delta("b c", ""));
                }
                ui.finished(&record(status, Some(3)));
                assert_eq!(ui.state.buckets.iter().sum::<u64>(), 3);
                if queued {
                    ui.delta(delta("b c", ""));
                }
                ui.tokenize(&tokens::test_tokenizer());
                assert_eq!(ui.state.buckets.iter().sum::<u64>(), 3);
            }
        }
    }

    #[test]
    fn finishing_counts_only_the_remainder_in_the_current_bucket() {
        let mut ui = live_ui();
        ui.delta(delta("b", "a"));
        ui.tokenize(&tokens::test_tokenizer());
        assert_eq!(ui.state.buckets.iter().sum::<u64>(), 2);
        ui.delta(delta(" c", ""));
        ui.state.bucket_at = Instant::now() - Duration::from_secs(1);
        ui.finished(&record(Status::Completed, Some(3)));
        assert_eq!(ui.state.buckets, VecDeque::from([2, 1]));
        ui.tokenize(&tokens::test_tokenizer());
        assert_eq!(ui.state.buckets, VecDeque::from([2, 1]));
    }

    #[test]
    fn missing_or_smaller_final_counts_do_not_add_tokens() {
        for generated in [None, Some(1)] {
            let mut ui = live_ui();
            for content in ["hel", "lo"] {
                ui.delta(delta(content, ""));
                ui.tokenize(&tokens::test_tokenizer());
            }
            assert_eq!(ui.state.buckets.iter().sum::<u64>(), 2);
            ui.finished(&record(Status::Completed, generated));
            assert_eq!(ui.state.buckets.iter().sum::<u64>(), 2);
        }
    }

    #[test]
    fn standard_terminal_keeps_complete_stable_headers() {
        let state = state(1);
        let area = Rect::new(0, 0, 80, 14);
        let mut previous = None;
        for _ in 0..64 {
            let mut buf = Buffer::empty(area);
            state.render(area, &mut buf);
            let header = lines(&buf)
                .into_iter()
                .find(|line| line.contains("request"))
                .unwrap();
            assert!(header.contains("reasoning"), "{header}");
            assert!(header.contains("answer"), "{header}");
            assert!(header.contains("last delta"), "{header}");
            assert!(!header.contains("of cap"), "{header}");
            if let Some(previous) = &previous {
                assert_eq!(&header, previous);
            }
            previous = Some(header);
        }
    }

    #[test]
    fn flight_columns_fit_available_width() {
        let state = state(1);
        for width in 0..=120 {
            let area = Rect::new(2, 1, width, 3);
            let mut buf = Buffer::empty(area);
            state.render_flight(area, &mut buf);
            let header = &lines(&buf)[0];
            for (label, needed) in [
                ("request", 8),
                ("elapsed", 18),
                ("ttft", 27),
                ("reasoning", 38),
                ("answer", 48),
                ("tok/s", 56),
                ("last delta", 71),
                ("of cap", 88),
            ] {
                if width >= needed {
                    assert!(header.contains(label), "width {width}: {header}");
                } else if needed > 8 {
                    assert!(!header.contains(label), "width {width}: {header}");
                }
            }
        }
    }

    #[test]
    fn hidden_count_includes_the_row_reserved_for_its_label() {
        let state = state(10);
        let area = Rect::new(2, 1, 116, 9);
        let mut buf = Buffer::empty(area);
        state.render_flight(area, &mut buf);
        let lines = lines(&buf);
        for id in 0..7 {
            assert!(lines[id + 1].starts_with(&format!("#{id} ")));
        }
        assert_eq!(lines[8].trim(), "… 3 more in flight");
    }

    #[test]
    fn every_live_request_is_visible_or_counted_in_the_footer() {
        for requests in [0, 1, 8, 10, 100] {
            let state = state(requests);
            for height in 1..=12 {
                for width in [20, 76, 116] {
                    let area = Rect::new(2, 1, width, height);
                    let mut buf = Buffer::empty(area);
                    state.render_flight(area, &mut buf);
                    let lines = lines(&buf);
                    let visible = lines.iter().filter(|line| line.starts_with('#')).count();
                    let footer = lines.last().unwrap().trim();
                    let hidden = footer
                        .strip_prefix("… ")
                        .map(|label| {
                            label
                                .strip_suffix(" more in flight")
                                .unwrap()
                                .parse::<usize>()
                                .unwrap()
                        })
                        .unwrap_or(0);
                    assert_eq!(
                        visible + hidden,
                        requests as usize,
                        "{requests} requests in {width}x{height}: {lines:?}"
                    );
                    if requests < height as u32 {
                        assert_eq!(hidden, 0);
                    } else {
                        assert!(hidden > 0);
                    }
                }
            }
        }
    }
}
