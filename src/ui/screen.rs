use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::cursor::{Hide, Show};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::{Frame, Terminal};
use std::io::{self, BufWriter, Stderr, Write};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static PANIC_HOOK: Once = Once::new();

pub struct Screen {
    terminal: Terminal<CrosstermBackend<BufWriter<Stderr>>>,
}

impl Screen {
    pub fn enter() -> io::Result<Self> {
        PANIC_HOOK.call_once(|| {
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                restore();
                previous(info);
            }));
        });
        enable_raw_mode()?;
        ACTIVE.store(true, Ordering::SeqCst);
        let mut writer = BufWriter::new(io::stderr());
        let terminal = execute!(writer, EnterAlternateScreen, Hide)
            .and_then(|()| Terminal::new(CrosstermBackend::new(writer)))
            .inspect_err(|_| restore())?;
        Ok(Self { terminal })
    }

    pub fn draw(&mut self, render: impl FnOnce(&mut Frame)) -> io::Result<()> {
        self.terminal.draw(render).map(drop)
    }

    /// Raw mode turns ctrl-c into a key press instead of SIGINT.
    pub fn interrupted(&mut self) -> bool {
        let mut interrupted = false;
        while event::poll(Duration::ZERO).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(key)) => {
                    interrupted |= key.kind == KeyEventKind::Press
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c');
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        interrupted
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let _ = self.terminal.backend_mut().flush();
        restore();
    }
}

fn restore() {
    if ACTIVE.swap(false, Ordering::SeqCst) {
        let _ = execute!(io::stderr(), LeaveAlternateScreen, Show);
        let _ = disable_raw_mode();
    }
}
