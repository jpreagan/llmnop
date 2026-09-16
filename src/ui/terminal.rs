use crate::style::content_style;
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::crossterm::cursor::{Hide, MoveDown, MoveToColumn, MoveUp, Show};
use ratatui::crossterm::queue;
use ratatui::crossterm::style::{PrintStyledContent, StyledContent};
use ratatui::crossterm::terminal::{self, Clear, ClearType as TerminalClear};
use ratatui::layout::{Position, Rect, Size};
use ratatui::text::{Line, Span};
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};
use std::io::{self, Write};

pub struct Inline<W: Write> {
    terminal: Terminal<Relative<W>>,
    height: u16,
}

impl<W: Write> Inline<W> {
    pub fn new(mut writer: W, height: u16) -> io::Result<Self> {
        let size = fit(terminal_size()?, height);
        writer.write_all(b"\r\n")?;
        let backend = Relative {
            writer,
            cursor: Position::ORIGIN,
            size,
        };
        let terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(height),
            },
        )?;
        Ok(Self { terminal, height })
    }

    fn size(&self) -> Size {
        self.terminal.backend().size
    }

    fn resize(&mut self) -> io::Result<()> {
        let size = fit(terminal_size().unwrap_or_else(|_| self.size()), self.height);
        if size != self.size() {
            let backend = self.terminal.backend_mut();
            backend.cursor.y = backend.cursor.y.min(size.height - 1);
            backend.size = size;
            self.terminal.resize(Rect::from((Position::ORIGIN, size)))?;
        }
        Ok(())
    }

    pub fn draw(&mut self, render: impl FnOnce(&mut Frame)) -> io::Result<()> {
        self.resize()?;
        self.terminal.draw(render)?;
        self.terminal
            .set_cursor_position((0, self.size().height - 1))?;
        self.terminal.backend_mut().flush()
    }

    pub fn insert_before(&mut self, lines: &[Line<'_>]) -> io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        self.resize()?;
        self.terminal.clear()?;
        let backend = self.terminal.backend_mut();
        for line in lines {
            for span in &line.spans {
                let content = span.content.replace(char::is_control, "");
                queue!(
                    backend.writer,
                    PrintStyledContent(StyledContent::new(
                        content_style(line.style.patch(span.style)),
                        content
                    ))
                )?;
            }
            backend.writer.write_all(b"\r\n")?;
            backend.cursor = Position::ORIGIN;
        }
        backend.append_lines(backend.size.height - 1)?;
        backend.flush()
    }
}

impl<W: Write> Drop for Inline<W> {
    fn drop(&mut self) {
        let _ = self.terminal.clear();
        let _ = self.terminal.show_cursor();
        let _ = self.terminal.backend_mut().flush();
    }
}

fn fit(size: Size, height: u16) -> Size {
    Size::new(size.width.max(1), size.height.min(height).max(1))
}

fn terminal_size() -> io::Result<Size> {
    terminal::size().map(|(width, height)| Size::new(width, height))
}

struct Relative<W> {
    writer: W,
    cursor: Position,
    size: Size,
}

impl<W: Write> Backend for Relative<W> {
    fn draw<'a, I>(&mut self, cells: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        for (x, y, cell) in cells {
            self.set_cursor_position((x, y))?;
            queue!(
                self.writer,
                PrintStyledContent(StyledContent::new(
                    content_style(cell.style()),
                    cell.symbol()
                ))
            )?;
            self.cursor.x = x.saturating_add(Span::raw(cell.symbol()).width() as u16);
        }
        Ok(())
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        let n = n.min(self.size.height.saturating_sub(self.cursor.y + 1));
        for _ in 0..n {
            self.writer.write_all(b"\r\n")?;
        }
        self.cursor = Position::new(0, self.cursor.y + n);
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        queue!(self.writer, Hide)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        queue!(self.writer, Show)
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(self.cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let position = position.into();
        if position.y < self.cursor.y {
            queue!(self.writer, MoveUp(self.cursor.y - position.y))?;
        } else if position.y > self.cursor.y {
            queue!(self.writer, MoveDown(position.y - self.cursor.y))?;
        }
        queue!(self.writer, MoveToColumn(position.x))?;
        self.cursor = position;
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.set_cursor_position(Position::ORIGIN)?;
        self.clear_region(ClearType::AfterCursor)
    }

    fn clear_region(&mut self, kind: ClearType) -> io::Result<()> {
        let kind = match kind {
            ClearType::All => return self.clear(),
            ClearType::AfterCursor => TerminalClear::FromCursorDown,
            ClearType::CurrentLine => TerminalClear::CurrentLine,
            ClearType::UntilNewLine => TerminalClear::UntilNewLine,
            ClearType::BeforeCursor => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "cannot clear before the inline viewport",
                ));
            }
        };
        queue!(self.writer, Clear(kind))
    }

    fn size(&self) -> io::Result<Size> {
        Ok(self.size)
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        Ok(WindowSize {
            columns_rows: self.size,
            pixels: Size::new(0, 0),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
