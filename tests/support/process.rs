use nix::fcntl::{FcntlArg, FdFlag, fcntl};
use nix::libc;
use nix::pty::{Winsize, openpty};
use nix::sys::termios::{Termios, tcgetattr};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const PREVIOUS_OUTPUT: &str = "previous output";
static SPAWN_LOCK: Mutex<()> = Mutex::new(());
type Reader = JoinHandle<io::Result<Vec<u8>>>;

pub struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub visible: String,
}

pub struct Terminal {
    child: Option<Child>,
    master: File,
    slave: Option<File>,
    original_modes: Termios,
    stdout: Option<Reader>,
    stderr: Option<Reader>,
    columns: u16,
    rows: u16,
}

impl Terminal {
    pub fn spawn(command: &mut Command, columns: u16, rows: u16, stdout_in_terminal: bool) -> Self {
        let spawning = SPAWN_LOCK.lock().unwrap();
        let size = Winsize {
            ws_row: rows,
            ws_col: columns,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let pty = openpty(Some(&size), None).unwrap();
        for fd in [&pty.master, &pty.slave] {
            fcntl(fd, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC)).unwrap();
        }
        let master = File::from(pty.master);
        let mut slave = File::from(pty.slave);
        let original_modes = tcgetattr(&slave).unwrap();
        writeln!(slave, "{PREVIOUS_OUTPUT}").unwrap();
        command
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave.try_clone().unwrap()))
            .stdout(if stdout_in_terminal {
                Stdio::from(slave.try_clone().unwrap())
            } else {
                Stdio::piped()
            });
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().unwrap();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        drop(spawning);
        let mut terminal = Self {
            child: Some(child),
            master,
            slave: Some(slave),
            original_modes,
            stdout: None,
            stderr: None,
            columns,
            rows,
        };
        if let Some(stdout) = terminal.child.as_mut().unwrap().stdout.take() {
            terminal.stdout = Some(read_in_background(stdout));
        }
        terminal.stderr = Some(read_in_background(terminal.master.try_clone().unwrap()));
        terminal
    }

    pub fn interrupt(&mut self) {
        self.master.write_all(b"\x03").unwrap();
    }

    pub fn finish(mut self) -> Output {
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = self.child.as_mut().unwrap().try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "benchmark did not exit within 30s"
            );
            thread::sleep(Duration::from_millis(10));
        };
        self.child.take();
        let final_modes = tcgetattr(&self.master).unwrap();
        self.slave.take();
        let stderr = self.stderr.take().unwrap().join().unwrap().unwrap();
        let stdout = self
            .stdout
            .take()
            .map_or_else(|| stderr.clone(), |reader| reader.join().unwrap().unwrap());
        assert_eq!(final_modes, self.original_modes, "terminal modes changed");

        let mut parser = vt100::Parser::new(self.rows, self.columns, 10_000);
        parser.process(&stderr);
        let screen = parser.screen();
        assert!(!screen.hide_cursor(), "cursor was left hidden");
        assert!(
            !screen.alternate_screen(),
            "alternate screen was left active"
        );
        parser.set_scrollback(usize::MAX);
        let history = u16::try_from(parser.screen().scrollback()).unwrap();
        parser.set_size(self.rows + history, self.columns);
        let visible = parser.screen().contents();
        assert!(
            visible.contains(PREVIOUS_OUTPUT),
            "existing terminal output was erased: {visible}"
        );
        Output {
            status,
            stdout,
            stderr,
            visible,
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.slave.take();
        for reader in [self.stdout.take(), self.stderr.take()]
            .into_iter()
            .flatten()
        {
            let _ = reader.join();
        }
    }
}

fn read_in_background(mut reader: impl Read + Send + 'static) -> Reader {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut buffer = [0; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => return Ok(bytes),
                Ok(count) => bytes.extend_from_slice(&buffer[..count]),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.raw_os_error() == Some(libc::EIO) => return Ok(bytes),
                Err(error) => return Err(error),
            }
        }
    })
}
