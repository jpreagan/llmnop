"""Exercise the CLI on a Unix PTY, with a local SSE server and no downloaded tokenizer."""

import codecs
import collections
import errno
import fcntl
import http.server
import json
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
import unicodedata
import unittest


BINARY = Path(sys.argv.pop(1) if len(sys.argv) > 1 else "target/debug/llmnop").resolve()
CSI = re.compile(r"\x1b\[([0-9;?]*)([A-Za-z])")
Run = collections.namedtuple("Run", "returncode screen stdout results")


class Screen:
    def __init__(self, width, height):
        self.width, self.height = width, height
        self.rows = [[" "] * width for _ in range(height)]
        self.history = []
        self.x = self.y = 0
        self.hidden = False
        self.pending = ""

    def newline(self):
        self.y += 1
        if self.y == self.height:
            self.history.append("".join(self.rows.pop(0)))
            self.rows.append([" "] * self.width)
            self.y -= 1

    def feed(self, text):
        self.pending += text
        while self.pending:
            char = self.pending[0]
            if char == "\x1b":
                match = CSI.match(self.pending)
                if match is None:
                    break
                args, command = match.groups()
                self.pending = self.pending[match.end():]
                value = int(args) if args.isdigit() else 1
                if command == "A":
                    self.y = max(0, self.y - value)
                elif command == "B":
                    self.y = min(self.height - 1, self.y + value)
                elif command == "G":
                    self.x = value - 1
                elif command == "J":
                    self.rows[self.y][self.x:] = [" "] * (self.width - self.x)
                    for y in range(self.y + 1, self.height):
                        self.rows[y] = [" "] * self.width
                elif args == "?25":
                    self.hidden = command == "l"
                continue
            self.pending = self.pending[1:]
            if char == "\r":
                self.x = 0
            elif char == "\n":
                self.newline()
            elif char >= " ":
                width = 2 if unicodedata.east_asian_width(char) in "WF" else 1
                if self.x + width > self.width:
                    self.x = 0
                    self.newline()
                self.rows[self.y][self.x] = char
                if width == 2:
                    self.rows[self.y][self.x + 1] = ""
                self.x += width

    def text(self):
        return "\n".join(self.history + ["".join(row) for row in self.rows])


class Endpoint(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        self.rfile.read(int(self.headers["Content-Length"]))
        if self.path.endswith("chat/completions"):
            delta = {"choices": [{"index": 0, "delta": {"content": "hello"}}]}
            done = "[DONE]"
        elif self.path.endswith("responses"):
            delta = {"type": "response.output_text.delta", "delta": "hello"}
            done = {"type": "response.completed", "response": {"status": "completed"}}
        else:
            delta = {"type": "content_block_delta", "delta": {"type": "text_delta", "text": "hello"}}
            done = {"type": "message_stop"}
        chunks = [f"data: {json.dumps(delta)}\n\n".encode(),
                  f"data: {done if isinstance(done, str) else json.dumps(done)}\n\n".encode()]
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(sum(map(len, chunks))))
        self.end_headers()
        try:
            self.wfile.write(chunks[0])
            self.wfile.flush()
            time.sleep(0.4)
            self.wfile.write(chunks[1])
        except (BrokenPipeError, ConnectionResetError):
            pass


class TerminalTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Endpoint)
        cls.server_thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.server_thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.server_thread.join()

    def run_cli(self, api="responses", fmt="json", mixed=True, width=120, height=35,
                interrupt=False, no_color=False):
        """Run one benchmark on a PTY and check what every run must leave behind."""
        directory = tempfile.TemporaryDirectory(prefix="llmnop-terminal-")
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        tokenizer = root / "tokenizer.json"
        tokenizer.write_text(json.dumps({
            "version": "1.0", "truncation": None, "padding": None, "added_tokens": [],
            "normalizer": None, "pre_tokenizer": {"type": "WhitespaceSplit"},
            "post_processor": None, "decoder": None,
            "model": {"type": "WordLevel", "vocab": {"[UNK]": 0, "hello": 1}, "unk_token": "[UNK]"},
        }))
        master, slave = pty.openpty()
        self.addCleanup(os.close, master)
        self.addCleanup(os.close, slave)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
        original = termios.tcgetattr(slave)
        os.write(slave, b"previous output\r\n")

        def setup():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        env = dict(os.environ, TERM="xterm-256color")
        env.pop("NO_COLOR", None)
        if no_color:
            env["NO_COLOR"] = "1"
        command = [str(BINARY), "--api", api, "--url", f"http://127.0.0.1:{self.server.server_port}/v1",
                   "--model", "test", "--tokenizer", str(tokenizer), "--input-tokens", "2",
                   "--output-cap", "16", "--requests", "2", "--format", fmt,
                   "--results-dir", str(root / "results")]
        process = subprocess.Popen(command, stdin=slave, stdout=subprocess.PIPE if mixed else slave,
                                   stderr=slave, preexec_fn=setup, env=env)
        self.addCleanup(lambda: process.poll() is None and process.kill())
        screen = Screen(width, height)
        decoder = codecs.getincrementaldecoder("utf8")()
        buffers = {master: bytearray()}
        if mixed:
            buffers[process.stdout.fileno()] = bytearray()
            self.addCleanup(process.stdout.close)
        started = time.monotonic()
        sent = changed_modes = False

        def read_ready(timeout):
            for fd in select.select(list(buffers), [], [], timeout)[0]:
                try:
                    chunk = os.read(fd, 65536)
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    chunk = b""
                buffers[fd].extend(chunk)
                if fd == master:
                    screen.feed(decoder.decode(chunk))

        while process.poll() is None:
            try:
                changed_modes |= termios.tcgetattr(slave) != original
            except termios.error:
                pass
            read_ready(0.005)
            if interrupt and not sent and time.monotonic() - started > 0.2:
                os.write(master, b"\x03")
                sent = True
            if time.monotonic() - started > 15:
                process.kill()
                process.wait()
                self.fail("benchmark failed to exit within 15 seconds")
        read_ready(0.01)
        tty = bytes(buffers[master])
        stdout = bytes(buffers[process.stdout.fileno()]) if mixed else tty
        text = screen.text()
        self.assertFalse(changed_modes, "dashboard changed terminal input modes")
        self.assertNotIn(b"\x1b[6n", stdout + tty, "cursor query escaped into output")
        self.assertNotIn(b"\x1b[?1049h", tty, "dashboard entered the alternate screen")
        self.assertFalse(screen.hidden, "cursor was left hidden")
        self.assertIn("previous output", text, "existing terminal output was erased")
        for border in "╭╮╰╯│":
            self.assertNotIn(border, text, "viewport rows were left behind")
        return Run(process.returncode, text, stdout, root / "results")

    def assert_completed(self, run):
        self.assertEqual(run.returncode, 0, run.screen)
        self.assertIn("✓ #0", run.screen)
        self.assertIn("✓ #1", run.screen)

    def assert_report(self, run, colored):
        self.assertLess(run.screen.index("✓ #1"), run.screen.index("llmnop "))
        start = re.search(rb"llmnop \d+\.\d+\.\d+", run.stdout)
        self.assertIsNotNone(start)
        self.assertEqual(b"\x1b" in run.stdout[start.start():], colored)

    def test_stdout_is_independent_of_terminal_stderr(self):
        for api in ("chat", "responses", "messages"):
            for fmt in ("json", "none"):
                with self.subTest(api=api, format=fmt):
                    run = self.run_cli(api=api, fmt=fmt)
                    self.assert_completed(run)
                    if fmt == "json":
                        summary = json.loads(run.stdout)
                        saved = json.loads(next(run.results.glob("*/summary.json")).read_text())
                        self.assertEqual(summary, saved)
                        self.assertEqual(summary["schema_version"], "3.0")
                    else:
                        self.assertEqual(run.stdout, b"")

    def test_interactive_trails_and_cleanup(self):
        for width, height in ((120, 35), (60, 6)):
            with self.subTest(size=(width, height)):
                run = self.run_cli(fmt="table", mixed=False, width=width, height=height)
                self.assert_completed(run)
                self.assert_report(run, colored=True)

    def test_no_color_report(self):
        run = self.run_cli(fmt="table", mixed=False, no_color=True)
        self.assert_completed(run)
        self.assert_report(run, colored=False)

    def test_startup_interrupt_is_preserved(self):
        run = self.run_cli(interrupt=True)
        self.assertEqual(run.returncode, 130)


if __name__ == "__main__":
    unittest.main()
