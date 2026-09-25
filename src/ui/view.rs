use super::{Activity, Outcome, Request, STALL, Stage, State};
use crate::benchmark::{Phase, seconds_per_token};
use crate::output::MetricStats;
use crate::style::{ACCENT, BAD, CONTENT, GOOD, REASONING, RULE, WARN, group, seconds};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Widget};
use std::time::{Duration, Instant};

const WIDE_COLUMNS: u16 = 120;
const WIDE_ROWS: u16 = 25;
const NARROW_COLUMNS: u16 = 80;
const NARROW_ROWS: u16 = 20;
const CHART_COLUMNS: u16 = 62;
const TOP_ROWS: (u16, u16) = (11, 24);
const TIMELINE_ROWS: u16 = 8;
const RATE_WINDOW: usize = 20;
const NOW_WINDOW: usize = 10;
const EIGHTHS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const TICK_STEPS: [u64; 7] = [15, 30, 60, 120, 300, 600, 1800];

pub fn render(state: &State, area: Rect, buf: &mut Buffer) {
    let mut view = View {
        state,
        now: Instant::now(),
        buf,
        area,
    };
    let tier = if area.width < NARROW_COLUMNS || area.height < NARROW_ROWS {
        Tier::Compact
    } else if area.width >= WIDE_COLUMNS && area.height >= WIDE_ROWS {
        Tier::Wide
    } else {
        Tier::Narrow
    };
    view.header(tier);
    if !matches!(state.stage, Stage::Run(_)) {
        view.preparing(tier);
    } else {
        match tier {
            Tier::Wide => view.wide(),
            Tier::Narrow => view.narrow(),
            Tier::Compact => view.compact(),
        }
    }
    view.footer(tier);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tier {
    Wide,
    Narrow,
    Compact,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Column {
    Request,
    State,
    Elapsed,
    Ttft,
    Reasoning,
    Content,
    Tokens,
    Rate,
    Since,
    Cap,
}

impl Column {
    fn spec(self) -> (&'static str, u16, bool) {
        match self {
            Self::Request => ("request", 7, false),
            Self::State => ("state", 12, false),
            Self::Elapsed => ("elapsed", 7, true),
            Self::Ttft => ("ttft", 6, true),
            Self::Reasoning => ("reasoning", 9, true),
            Self::Content => ("content", 7, true),
            Self::Tokens => ("tokens", 6, true),
            Self::Rate => ("tok/s", 5, true),
            Self::Since => ("since last", 10, true),
            Self::Cap => ("of cap", 6, true),
        }
    }

    /// The widest set that fits, dropping less important columns first.
    fn fitting(width: u16, detailed: bool) -> &'static [Column] {
        use Column::*;
        const SETS: [&[Column]; 7] = [
            &[
                Request, State, Elapsed, Ttft, Reasoning, Content, Rate, Since, Cap,
            ],
            &[Request, State, Elapsed, Ttft, Reasoning, Content, Rate, Cap],
            &[Request, State, Elapsed, Ttft, Tokens, Rate, Cap],
            &[Request, State, Elapsed, Ttft, Tokens, Rate],
            &[Request, State, Elapsed, Ttft, Tokens],
            &[Request, State, Elapsed, Tokens],
            &[Request, State, Elapsed],
        ];
        let sets = if detailed { &SETS[..] } else { &SETS[2..] };
        sets.iter()
            .find(|set| {
                let total: u16 = set.iter().map(|c| c.spec().1 + 2).sum();
                total - 2 <= width
            })
            .copied()
            .unwrap_or(SETS[6])
    }
}

#[derive(Clone, Copy)]
enum Bar {
    Empty,
    Value(f64, Style),
    Failed,
}

struct View<'a> {
    state: &'a State,
    now: Instant,
    buf: &'a mut Buffer,
    area: Rect,
}

impl View<'_> {
    fn put<'s>(&mut self, x: u16, y: u16, line: impl Into<Line<'s>>) -> u16 {
        let line = line.into();
        if y >= self.area.bottom() || x >= self.area.right() {
            return x;
        }
        self.buf.set_line(x, y, &line, self.area.right() - x).0
    }

    fn put_right<'s>(&mut self, right: u16, y: u16, line: impl Into<Line<'s>>) -> u16 {
        let line = line.into();
        let x = right.saturating_sub(line.width() as u16).max(self.area.x);
        self.put(x, y, line);
        x
    }

    fn panel(&mut self, area: Rect, title: &str, extra: Vec<Span<'static>>) -> Rect {
        let area = area.intersection(self.area);
        let mut spans = vec!["─ ".fg(RULE), title.to_string().bold()];
        spans.extend(extra);
        spans.push(" ".into());
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(RULE))
            .title(Line::from(spans));
        let inner = block.inner(area);
        block.render(area, self.buf);
        inner
    }

    fn elapsed(&self) -> Duration {
        self.now.duration_since(self.state.stage_started)
    }

    fn header(&mut self, tier: Tier) {
        let (x, y, right) = (self.area.x, self.area.y, self.area.right());
        let s = self.state;
        let mut end = if tier == Tier::Compact {
            self.put(x, y, "llmnop".fg(ACCENT).bold())
        } else {
            self.put(x, y, " llmnop ".fg(ACCENT).reversed().bold())
        };
        end = self.put(end + 2, y, s.model.clone().bold());
        let detail = match tier {
            Tier::Wide => vec![s.api.clone().dim(), " · ".fg(RULE), s.url.clone().dim()],
            Tier::Narrow => vec![
                s.api.clone().dim(),
                " · ".fg(RULE),
                s.settings
                    .split(" · ")
                    .next()
                    .unwrap_or("")
                    .to_string()
                    .dim(),
            ],
            Tier::Compact => Vec::new(),
        };
        end = self.put(end + 2, y, detail);
        let status = match tier {
            Tier::Wide => s.settings.clone().dim(),
            _ => clock(self.elapsed()).bold(),
        };
        if end + 2 + status.width() as u16 <= right {
            self.put_right(right, y, status);
        }
    }

    fn footer(&mut self, tier: Tier) {
        let y = self.area.bottom() - 1;
        let x = self.area.x + u16::from(tier != Tier::Compact);
        let end = if self.state.cancelling() {
            self.put(x, y, "cancelling…".fg(WARN).bold())
        } else {
            self.put(x, y, vec!["ctrl-c".bold(), " cancel".dim()])
        };
        let note = "report prints to the terminal when the run ends";
        let right = self.area.right().saturating_sub(1);
        if tier != Tier::Compact && self.state.prints_report && end + 2 + note.len() as u16 <= right
        {
            self.put_right(right, y, note.dim());
        }
    }

    fn preparing(&mut self, tier: Tier) {
        let elapsed = self.elapsed();
        let spinner = SPINNER[(elapsed.as_millis() / 100) as usize % SPINNER.len()];
        let x = self.area.x + if tier == Tier::Compact { 0 } else { 2 };
        let label = self.state.stage_label();
        self.put(
            x,
            self.area.y + 2,
            vec![
                format!("{spinner} ").fg(ACCENT),
                label.bold(),
                format!("  {}", clock(elapsed)).dim(),
            ],
        );
    }

    fn progress(&mut self, x: u16, y: u16, right: u16, tier: Tier) {
        let s = self.state;
        let done = s.tally.started;
        let mut spans = vec![done.to_string().bold()];
        if tier == Tier::Compact {
            spans.push(format!("/{}", s.total).dim());
        } else {
            spans.push(format!(" / {}", s.total).dim());
        }
        if tier == Tier::Wide {
            spans.push("   ".into());
            spans.push(clock(self.elapsed()).bold());
            spans.push(" elapsed".dim());
        }
        if let Some(eta) = s.eta() {
            spans.push(format!("  ~{}", clock(eta)).bold());
            spans.push(" left".dim());
        }
        let text = Line::from(spans);
        let width = right
            .saturating_sub(x)
            .saturating_sub(text.width() as u16 + 2);
        let ratio = if s.total == 0 {
            0.0
        } else {
            (done as f64 / s.total as f64).min(1.0)
        };
        let filled = (f64::from(width) * ratio).round() as usize;
        let color = if s.cancelling() { WARN } else { ACCENT };
        let end = self.put(
            x,
            y,
            vec![
                "━".repeat(filled).fg(color),
                "─".repeat(width as usize - filled).fg(RULE),
            ],
        );
        self.put(end + 2, y, text);
    }

    fn outcomes(&self) -> Vec<Span<'static>> {
        let s = self.state;
        let t = &s.tally;
        let mut spans = Vec::new();
        let mut count = |n: usize, what: &str, style: Style| {
            if n > 0 {
                if !spans.is_empty() {
                    spans.push("  ".into());
                }
                spans.push(Span::styled(n.to_string(), style.bold()));
                spans.push(format!(" {what}").dim());
            }
        };
        count(
            t.completed - t.completed_at_output_limit,
            "done",
            Style::new().fg(GOOD),
        );
        count(t.failed, "failed", Style::new().fg(BAD));
        count(t.timed_out, "timed out", Style::new().fg(BAD));
        count(t.cancelled, "cancelled", Style::new().fg(WARN));
        count(t.completed_at_output_limit, "capped", Style::new().fg(WARN));
        count(s.in_flight().count(), "in flight", Style::new());
        count(s.queued(), "queued", Style::new());
        spans
    }

    fn phase_label(&self) -> Span<'static> {
        if self.state.cancelling() {
            "cancelling".fg(WARN)
        } else if self.state.stage == Stage::Run(Phase::Warmup) {
            "warmup".fg(ACCENT)
        } else {
            "progress".dim()
        }
    }

    fn wide(&mut self) {
        let a = self.area;
        let label = self.phase_label();
        self.put(a.x + 2, a.y + 2, label);
        self.progress(a.x + 12, a.y + 2, a.right() - 1, Tier::Wide);
        let outcomes = self.outcomes();
        self.put(a.x + 12, a.y + 3, outcomes);

        let body = a.height - 6;
        let top = (body * 54 / 100)
            .clamp(TOP_ROWS.0, TOP_ROWS.1)
            .min(body - TIMELINE_ROWS);
        let charts = a.width.saturating_sub(98).clamp(40, CHART_COLUMNS);
        let requests = Rect::new(a.x, a.y + 5, a.width - charts, top);
        self.requests(requests, true);
        let rate_rows = ((f64::from(top) * 0.52).round() as u16).max(6);
        let chart_x = a.x + a.width - charts;
        self.throughput(Rect::new(chart_x, a.y + 5, charts, rate_rows));
        self.ttft(Rect::new(
            chart_x,
            a.y + 5 + rate_rows,
            charts,
            top - rate_rows,
        ));
        let timeline_y = a.y + 5 + top;
        self.timeline(
            Rect::new(a.x, timeline_y, a.width, a.bottom() - 1 - timeline_y),
            8,
        );
    }

    fn narrow(&mut self) {
        let a = self.area;
        self.progress(a.x + 1, a.y + 2, a.right() - 1, Tier::Narrow);
        let outcomes = self.outcomes();
        let end = self.put(a.x + 1, a.y + 3, outcomes);
        let (now, run) = self.rates();
        let rates = Line::from(vec![
            format!("{now:.1}").fg(ACCENT).bold(),
            " tok/s now".dim(),
            " · ".fg(RULE),
            format!("{run:.1}").bold(),
            " run".dim(),
        ]);
        if end + 2 + rates.width() as u16 <= a.right() {
            self.put_right(a.right(), a.y + 3, rates);
        }
        let body = a.height - 6;
        let slots = self.state.total.clamp(1, 64) as u16;
        let flight = (slots.min(self.concurrency()) + 3)
            .max(4)
            .min(body - TIMELINE_ROWS);
        self.requests(Rect::new(a.x, a.y + 5, a.width, flight), false);
        let timeline_y = a.y + 5 + flight;
        self.timeline(
            Rect::new(a.x, timeline_y, a.width, a.bottom() - 1 - timeline_y),
            4,
        );
    }

    fn concurrency(&self) -> u16 {
        self.state
            .settings
            .split(" · ")
            .next()
            .and_then(|s| s.strip_prefix("concurrency "))
            .and_then(|n| n.parse().ok())
            .unwrap_or(4)
    }

    fn compact(&mut self) {
        let a = self.area;
        let right = a.right();
        self.progress(a.x, a.y + 1, right, Tier::Compact);
        let outcomes = self.outcomes();
        self.put(a.x, a.y + 2, outcomes);

        let (now, run) = self.rates();
        let rates = Line::from(vec![
            format!("{now:.1}").fg(ACCENT).bold(),
            " now  ".dim(),
            format!("{run:.1}").bold(),
            " run".dim(),
        ]);
        let spark = right
            .saturating_sub(a.x + 6)
            .saturating_sub(rates.width() as u16 + 1);
        self.put(a.x, a.y + 3, "tok/s".dim());
        let values = self.smoothed(spark as usize);
        let max = nice(values.iter().copied().fold(0.0, f64::max));
        let glyphs: String = values
            .iter()
            .map(|v| EIGHTHS[((v / max * 8.0).round() as usize).clamp(1, 8)])
            .collect();
        self.put(a.x + 6, a.y + 3, glyphs.fg(ACCENT));
        self.put_right(right, a.y + 3, rates);

        let mut ttft = vec!["ttft  ".dim()];
        let (p50, last) = self.ttft_summary();
        if let Some(last) = last {
            ttft.extend([seconds_of(last).bold(), " last  ".dim()]);
        }
        if let Some(p50) = p50 {
            ttft.extend([seconds_of(p50).bold(), " p50".dim()]);
        }
        self.put(a.x, a.y + 4, ttft);

        let first = a.y + 6;
        let rows = a.bottom().saturating_sub(1).saturating_sub(first) as usize;
        let flying: Vec<_> = self.state.in_flight().collect();
        let shown = if flying.len() > rows {
            rows.saturating_sub(1)
        } else {
            flying.len()
        };
        for (i, (id, request)) in flying.iter().take(shown).enumerate() {
            let (state, style) = self.state_cell(request);
            let mut spans = vec![
                format!("#{id}").bold(),
                "  ".into(),
                Span::styled(format!("{state:<12}"), style),
                format!("{:>7}", seconds_of(self.now - request.started)).into(),
            ];
            let generated = request.generated();
            if generated > 0 {
                spans.push(format!("  {:>5} tok", group(generated)).dim());
                if let Some(cap) = request.cap {
                    spans.push(format!("  {}% of cap", percent(generated, cap)).dim());
                }
            }
            self.put(a.x, first + i as u16, spans);
        }
        if shown < flying.len() && rows > 0 {
            self.put(
                a.x,
                first + shown as u16,
                format!("+{} more in flight", flying.len() - shown).dim(),
            );
        }
    }

    fn state_cell(&self, request: &Request) -> (String, Style) {
        match &request.outcome {
            Some(Outcome::Done) => ("✓ done".into(), Style::new().fg(GOOD)),
            Some(Outcome::Capped) => ("⊘ capped".into(), Style::new().fg(WARN)),
            Some(Outcome::Failed(why)) => (format!("✗ {why}"), Style::new().fg(BAD)),
            Some(Outcome::TimedOut) => ("✗ timed out".into(), Style::new().fg(BAD)),
            Some(Outcome::Cancelled) => ("– cancelled".into(), Style::new().fg(WARN)),
            None => match request.last {
                None => ("waiting".into(), Style::new().dim()),
                Some(last) if self.now - last >= STALL => (
                    format!("stalled {}", short(self.now - last)),
                    Style::new().fg(WARN).bold(),
                ),
                Some(_) => match request.activity.last().map(|(_, a)| *a) {
                    Some(Activity::Content) => ("content".into(), Style::new().fg(CONTENT)),
                    _ => ("reasoning".into(), Style::new().fg(REASONING)),
                },
            },
        }
    }

    fn cell(&self, column: Column, id: u32, request: &Request) -> Span<'static> {
        let flying = request.in_flight();
        let missing = || "—".dim();
        let failed = request.outcome.as_ref().is_some_and(Outcome::failed);
        let tokens = |n: u64, color| {
            if failed && n == 0 {
                missing()
            } else if flying {
                group(n).fg(color)
            } else {
                group(n).into()
            }
        };
        match column {
            Column::Request if flying => format!("#{id}").bold(),
            Column::Request => format!("#{id}").into(),
            Column::State => {
                let (text, style) = self.state_cell(request);
                Span::styled(text, style)
            }
            Column::Elapsed => {
                seconds_of(request.ended.unwrap_or(self.now) - request.started).into()
            }
            Column::Ttft => request
                .ttft()
                .map_or_else(missing, |t| seconds_of(t).into()),
            Column::Reasoning => tokens(request.reasoning, REASONING),
            Column::Content => tokens(request.content, CONTENT),
            Column::Tokens => tokens(request.generated(), ACCENT),
            Column::Rate => {
                let rate = if flying {
                    request
                        .first
                        .zip(request.last)
                        .and_then(|(a, b)| seconds_per_token(b - a, request.generated()))
                        .map(|s| 1.0 / s)
                } else {
                    request.rate
                };
                rate.map_or_else(missing, |r| format!("{r:.1}").into())
            }
            Column::Since => match request.last.filter(|_| flying) {
                Some(last) if self.now - last >= STALL => {
                    seconds_of(self.now - last).fg(WARN).bold()
                }
                Some(last) => seconds_of(self.now - last).into(),
                None => "".into(),
            },
            Column::Cap => match request.cap {
                Some(cap) if !failed => format!("{}%", percent(request.generated(), cap)).into(),
                _ => "".into(),
            },
        }
    }

    fn row(&mut self, x: u16, y: u16, columns: &[Column], cells: Vec<Span<'static>>) {
        let mut cx = x;
        for (column, cell) in columns.iter().zip(cells) {
            let (_, width, right) = column.spec();
            let text: String = cell.content.chars().take(width as usize).collect();
            let cell = Span::styled(text, cell.style);
            if right {
                self.put_right(cx + width, y, cell);
            } else {
                self.put(cx, y, cell);
            }
            cx += width + 2;
        }
    }

    fn requests(&mut self, area: Rect, recent: bool) {
        let title = if recent { "Requests" } else { "In flight" };
        let inner = self.panel(area, title, Vec::new());
        if inner.width < 6 || inner.height == 0 {
            return;
        }
        let inner = Rect::new(inner.x + 2, inner.y, inner.width - 4, inner.height);
        let columns = Column::fitting(inner.width, recent);
        let header = columns.iter().map(|c| c.spec().0.dim()).collect();
        self.row(inner.x, inner.y, columns, header);

        let capacity = inner.height as usize - 1;
        let flying: Vec<_> = self.state.in_flight().collect();
        let shown = if flying.len() > capacity {
            capacity.saturating_sub(1)
        } else {
            flying.len()
        };
        let mut y = inner.y + 1;
        for (id, request) in flying.iter().take(shown) {
            let cells = columns
                .iter()
                .map(|c| self.cell(*c, **id, request))
                .collect();
            self.row(inner.x, y, columns, cells);
            y += 1;
        }
        if shown < flying.len() {
            self.put(
                inner.x,
                y,
                format!("+{} more in flight", flying.len() - shown).dim(),
            );
            return;
        }
        let room = inner.bottom().saturating_sub(y + 1) as usize;
        if !recent || room == 0 {
            return;
        }
        self.put(
            inner.x,
            y,
            vec![
                "recent ".dim(),
                "─".repeat(inner.width as usize - 7).fg(RULE),
            ],
        );
        let mut finished: Vec<_> = self
            .state
            .requests
            .iter()
            .filter(|(_, r)| !r.in_flight())
            .collect();
        finished.sort_by_key(|(_, r)| std::cmp::Reverse(r.ended));
        for (i, (id, request)) in finished.into_iter().take(room).enumerate() {
            let cells = columns
                .iter()
                .map(|c| {
                    let cell = self.cell(*c, *id, request);
                    if *c == Column::State {
                        cell
                    } else {
                        cell.dim()
                    }
                })
                .collect();
            self.row(inner.x, y + 1 + i as u16, columns, cells);
        }
    }

    /// Tokens per second in each completed second of the phase.
    fn seconds(&self) -> &[u64] {
        let buckets = &self.state.buckets;
        &buckets[..buckets.len().saturating_sub(1)]
    }

    fn rates(&self) -> (f64, f64) {
        let seconds = self.seconds();
        let recent = &seconds[seconds.len().saturating_sub(NOW_WINDOW)..];
        let now = if recent.is_empty() {
            0.0
        } else {
            recent.iter().sum::<u64>() as f64 / recent.len() as f64
        };
        let elapsed = self.elapsed().as_secs_f64();
        let total: u64 = self.state.buckets.iter().sum();
        let run = if elapsed > 0.0 {
            total as f64 / elapsed
        } else {
            0.0
        };
        (now, run)
    }

    /// The trailing average rate at evenly spaced points across the phase.
    fn smoothed(&self, bins: usize) -> Vec<f64> {
        let seconds = self.seconds();
        if seconds.is_empty() {
            return Vec::new();
        }
        let span = seconds.len() as f64;
        (0..bins)
            .map(|i| {
                let end = ((i + 1) as f64 * span / bins as f64).floor() as usize;
                let start = end.saturating_sub(RATE_WINDOW);
                let end = end.max(start + 1).min(seconds.len());
                let window = &seconds[start..end];
                window.iter().sum::<u64>() as f64 / window.len().max(1) as f64
            })
            .collect()
    }

    fn throughput(&mut self, area: Rect) {
        let (now, run) = self.rates();
        let inner = self.panel(
            area,
            "Output tokens/s",
            vec![
                format!("  {now:.1} now").fg(ACCENT),
                format!(" · {run:.1} run").dim(),
            ],
        );
        let width = inner.width.saturating_sub(8) as usize;
        let values = self.smoothed(width);
        let max = nice(values.iter().copied().fold(0.0, f64::max));
        let bars: Vec<_> = values
            .into_iter()
            .map(|v| Bar::Value(v, Style::new().fg(ACCENT)))
            .collect();
        let end = clock(Duration::from_secs(self.seconds().len() as u64));
        self.chart(inner, &bars, max, trim, ("0".into(), None, end), width);
    }

    fn ttft_summary(&self) -> (Option<Duration>, Option<Duration>) {
        let mut done: Vec<_> = self
            .state
            .requests
            .values()
            .filter(|r| r.outcome.as_ref().is_some_and(Outcome::succeeded))
            .filter_map(|r| Some((r.ended?, r.ttft()?)))
            .collect();
        done.sort_by_key(|(ended, _)| *ended);
        let stats = MetricStats::new(done.iter().map(|(_, t)| t.as_secs_f64()).collect());
        (
            stats.p50.map(Duration::from_secs_f64),
            done.last().map(|(_, t)| *t),
        )
    }

    fn ttft(&mut self, area: Rect) {
        let inner_width = area.width.saturating_sub(10) as usize;
        let requests: Vec<_> = self.state.requests.iter().collect();
        let bars: Vec<Bar> = requests
            .iter()
            .map(|(_, r)| match (&r.outcome, r.ttft()) {
                (Some(o), _) if o.failed() => Bar::Failed,
                (Some(o), Some(t)) if o.succeeded() => {
                    Bar::Value(t.as_secs_f64(), Style::new().fg(WARN))
                }
                (None, Some(t)) => Bar::Value(t.as_secs_f64(), Style::new().dim()),
                _ => Bar::Empty,
            })
            .collect();
        let per = bars.len().div_ceil(inner_width.max(1)).max(1);
        let extra = if per > 1 {
            vec![format!("  {per} requests per bar · median").dim()]
        } else {
            let (p50, last) = self.ttft_summary();
            let mut extra = Vec::new();
            if let Some(p50) = p50 {
                extra.push(format!("  p50 {}", seconds_of(p50)).dim());
            }
            if let Some(last) = last {
                extra.push(format!(" · last {}", seconds_of(last)).dim());
            }
            extra
        };
        let bars: Vec<Bar> = bars.chunks(per).map(median).collect();
        let inner = self.panel(area, "Time to first token", extra);
        let max = nice(
            bars.iter()
                .filter_map(|b| match b {
                    Bar::Value(v, _) => Some(*v),
                    _ => None,
                })
                .fold(0.0, f64::max),
        );
        let label = |i: usize| format!("#{}", requests[(i * per).min(requests.len() - 1)].0);
        let labels = if bars.is_empty() {
            (String::new(), None, String::new())
        } else {
            let middle = (bars.len() >= 24).then(|| (bars.len() / 2, label(bars.len() / 2)));
            (label(0), middle, label(bars.len() - 1))
        };
        self.chart(
            inner,
            &bars,
            max,
            |v| format!("{}s", trim(v)),
            labels,
            inner_width,
        );
    }

    fn chart(
        &mut self,
        inner: Rect,
        bars: &[Bar],
        max: f64,
        label: impl Fn(f64) -> String,
        (first, middle, last): (String, Option<(usize, String)>, String),
        width: usize,
    ) {
        if inner.height < 3 || inner.width < 10 {
            return;
        }
        let rows = inner.height - 2;
        let axis = inner.x + 6;
        let x = axis + 1;
        self.put_right(axis, inner.y, label(max).dim());
        self.put_right(axis, inner.y + rows - 1, "0".dim());
        if rows >= 5 {
            self.put_right(axis, inner.y + (rows - 1) / 2, label(max / 2.0).dim());
        }
        for row in 0..rows {
            self.put(axis, inner.y + row, "┤".fg(RULE));
        }
        self.put(
            axis,
            inner.y + rows,
            format!("└{}", "─".repeat(width)).fg(RULE),
        );
        for (i, bar) in bars.iter().take(width).enumerate() {
            let column = x + i as u16;
            match *bar {
                Bar::Empty => {}
                Bar::Failed => {
                    self.put(column, inner.y + rows - 1, "×".fg(BAD));
                }
                Bar::Value(v, style) => {
                    let mut units = (v / max * f64::from(rows) * 8.0).round() as u16;
                    if v > 0.0 {
                        units = units.max(1);
                    }
                    for row in (0..rows).rev() {
                        if units == 0 {
                            break;
                        }
                        let unit = units.min(8);
                        self.put(
                            column,
                            inner.y + row,
                            Span::styled(EIGHTHS[unit as usize], style),
                        );
                        units -= unit;
                    }
                }
            }
        }
        let y = inner.y + rows + 1;
        let end = if bars.is_empty() {
            x + width as u16
        } else {
            x + bars.len().min(width) as u16
        };
        let mut free = self.put(x, y, first.dim()) + 1;
        if let Some((i, text)) = middle {
            let at = x + i as u16;
            if at >= free && at + text.len() as u16 + 1 < end.saturating_sub(last.len() as u16) {
                free = self.put(at, y, text.dim()) + 1;
            }
        }
        if end >= free + last.len() as u16 {
            self.put_right(end, y, last.dim());
        }
    }

    /// Recent history: about three typical request lifetimes, in round steps.
    fn window(&self) -> Duration {
        let stats = MetricStats::new(self.state.latency.clone());
        let basis = match stats.p50 {
            Some(ms) => 3.0 * ms / 1000.0,
            None => {
                let oldest = self
                    .state
                    .in_flight()
                    .map(|(_, r)| (self.now - r.started).as_secs_f64())
                    .fold(0.0, f64::max);
                1.5 * oldest
            }
        };
        let seconds = ((basis / 30.0).ceil() * 30.0).clamp(60.0, 1800.0);
        Duration::from_secs_f64(seconds)
    }

    fn timeline(&mut self, area: Rect, max_ticks: u64) {
        let span = self.window();
        let inner = self.panel(
            area,
            "Timeline",
            vec![format!("  last {}", short(span)).dim()],
        );
        if inner.height < 3 || inner.width < 20 {
            return;
        }
        let rows = (inner.height - 2) as usize;
        let start = self
            .now
            .checked_sub(span)
            .unwrap_or(self.state.stage_started);
        let visible: Vec<_> = self
            .state
            .requests
            .iter()
            .filter(|(_, r)| r.ended.is_none_or(|end| end > start))
            .collect();
        let visible = &visible[visible.len().saturating_sub(rows)..];
        let label_width = self
            .state
            .requests
            .keys()
            .next_back()
            .map_or(2, |id| format!("#{id}").len() as u16 + 2);
        let x = inner.x + 2 + label_width;
        let width = inner.right().saturating_sub(x + 2);
        if width < 10 {
            return;
        }
        let cell = span.as_secs_f64() / f64::from(width);
        let now = self.now;
        for (row, (id, request)) in visible.iter().enumerate() {
            let y = inner.y + row as u16;
            let label = format!("#{id}");
            if request.in_flight() {
                self.put(inner.x + 2, y, label.bold());
            } else {
                self.put(inner.x + 2, y, label.dim());
            }
            let end = request.ended.unwrap_or(now);
            let failed = request.outcome.as_ref().is_some_and(Outcome::failed);
            let at = |t: Instant| -> Option<Activity> {
                if t < request.started || t >= end {
                    return None;
                }
                if request.in_flight() && request.last.is_some_and(|l| now - l >= STALL && t >= l) {
                    return Some(Activity::Stalled);
                }
                let i = request.activity.partition_point(|(from, _)| *from <= t);
                Some(request.activity[i.saturating_sub(1)].1)
            };
            let mut glyphs = Vec::with_capacity(width as usize);
            for i in 0..width {
                let from = start + Duration::from_secs_f64(f64::from(i) * cell);
                let to = from + Duration::from_secs_f64(cell);
                let middle = from + Duration::from_secs_f64(cell / 2.0);
                let activity = at(middle).or_else(|| {
                    // Keep requests shorter than one cell visible.
                    (request.started >= from && request.started < to)
                        .then(|| at(request.started))?
                });
                let glyph = match activity {
                    None => " ".into(),
                    Some(_) if failed => "─".fg(BAD),
                    Some(Activity::Waiting) => "─".dim(),
                    Some(Activity::Reasoning) => "━".fg(REASONING),
                    Some(Activity::Content) => "━".fg(CONTENT),
                    Some(Activity::Stalled) => "┅".fg(WARN),
                };
                glyphs.push(glyph);
            }
            if failed && end > start {
                let i = (((end - start).as_secs_f64() / cell) as usize).min(width as usize - 1);
                glyphs[i] = "✗".fg(BAD);
            }
            self.put(x, y, glyphs);
        }

        let y = inner.bottom() - 2;
        self.put(x, y, "─".repeat(width as usize).fg(RULE));
        let total = span.as_secs();
        let step = TICK_STEPS
            .into_iter()
            .find(|step| total / step <= max_ticks)
            .unwrap_or(1800);
        let mut back = 0;
        while back <= total {
            let offset =
                ((f64::from(width - 1)) * (1.0 - back as f64 / total as f64)).round() as u16;
            self.put(x + offset, y, "┴".fg(RULE));
            let label = if back == 0 {
                "now".to_string()
            } else {
                format!("-{}", short(Duration::from_secs(back)).replace(' ', ""))
            };
            let len = label.len() as u16;
            let lx = (x + offset)
                .saturating_sub(len / 2)
                .clamp(x, x + width - len);
            self.put(lx, y + 1, label.dim());
            back += step;
        }
    }
}

fn median(chunk: &[Bar]) -> Bar {
    let mut values: Vec<f64> = chunk
        .iter()
        .filter_map(|b| match b {
            Bar::Value(v, _) => Some(*v),
            _ => None,
        })
        .collect();
    if values.is_empty() {
        return if chunk.iter().any(|b| matches!(b, Bar::Failed)) {
            Bar::Failed
        } else {
            Bar::Empty
        };
    }
    if chunk.len() == 1 {
        return chunk[0];
    }
    values.sort_unstable_by(f64::total_cmp);
    Bar::Value(values[values.len() / 2], Style::new().fg(WARN))
}

/// Rounds an axis maximum up to 1, 2, 3, 4, 5, 6 or 8 times a power of ten.
fn nice(max: f64) -> f64 {
    if max <= 0.0 {
        return 1.0;
    }
    let power = 10f64.powf(max.log10().floor());
    let scaled = max / power;
    let step = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0]
        .into_iter()
        .find(|step| scaled <= step + 1e-9)
        .unwrap_or(10.0);
    step * power
}

fn trim(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else if value >= 1.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}

fn percent(generated: u64, cap: u32) -> u64 {
    generated * 100 / u64::from(cap.max(1))
}

fn seconds_of(d: Duration) -> String {
    seconds(d.as_secs_f64() * 1000.0)
}

fn clock(d: Duration) -> String {
    let s = d.as_secs();
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m {:02}s", s / 60, s % 60)
    } else {
        format!("{}h {:02}m", s / 3600, s / 60 % 60)
    }
}

fn short(d: Duration) -> String {
    let s = d.as_secs();
    match (s / 60, s % 60) {
        (0, s) => format!("{s}s"),
        (m, 0) => format!("{m}m"),
        (m, s) => format!("{m}m {s}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::busy;
    use super::*;

    fn frame(width: u16, height: u16) -> Buffer {
        let state = busy();
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        render(&state, area, &mut buf);
        buf
    }

    #[test]
    fn every_terminal_size_renders_within_bounds() {
        for width in 1..=160 {
            for height in 1..=50 {
                frame(width, height);
            }
        }
    }
}
