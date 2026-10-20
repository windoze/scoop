use crate::Protocol;
use crate::child::OwnedChild;
use crate::reader::{self, Message};
use anyhow::{Context, Result, bail, ensure};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct Terminal {
    pub parser: vt100::Parser<Protocol>,
    pub raw: Vec<u8>,
    pub interrupted: Arc<AtomicBool>,
    pub initial_termios: nix::sys::termios::Termios,
    master: Box<dyn MasterPty + Send>,
    child: OwnedChild,
    writer: Box<dyn Write + Send>,
    receiver: Receiver<Message>,
    reader: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    eof: bool,
    deadline: Instant,
}

impl Terminal {
    pub fn spawn(mut command: CommandBuilder, columns: u16, rows: u16) -> Result<Self> {
        Self::spawn_configured(&mut command, columns, rows, false)
    }

    pub fn spawn_raw(mut command: CommandBuilder, columns: u16, rows: u16) -> Result<Self> {
        Self::spawn_configured(&mut command, columns, rows, true)
    }

    fn spawn_configured(
        command: &mut CommandBuilder,
        columns: u16,
        rows: u16,
        raw: bool,
    ) -> Result<Self> {
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env_remove("NO_COLOR");
        let pair = native_pty_system().openpty(size(columns, rows))?;
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let fd = pair.master.as_raw_fd().context("PTY master has no fd")?;
        let mut initial_termios = pair.master.get_termios().context("initial PTY termios")?;
        if raw {
            nix::sys::termios::cfmakeraw(&mut initial_termios);
            let borrowed = unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) };
            nix::sys::termios::tcsetattr(
                borrowed,
                nix::sys::termios::SetArg::TCSANOW,
                &initial_termios,
            )?;
        }
        let child = OwnedChild {
            child: pair.slave.spawn_command(command.clone())?,
            status: None,
            group_finished: false,
        };
        drop(pair.slave);
        let stop = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        let reader_stop = Arc::clone(&stop);
        let reader = std::thread::Builder::new()
            .name("terminal-output".into())
            .spawn(move || reader::run(reader, fd, reader_stop, sender))?;
        Ok(Self {
            parser: vt100::Parser::new_with_callbacks(rows, columns, 0, Protocol::default()),
            raw: Vec::new(),
            interrupted: Arc::new(AtomicBool::new(false)),
            initial_termios,
            master: pair.master,
            child,
            writer,
            receiver,
            reader: Some(reader),
            stop,
            eof: false,
            deadline: Instant::now() + Duration::from_secs(30),
        })
    }

    pub fn send(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes).context("send PTY input")?;
        self.writer.flush().context("flush PTY input")
    }

    pub fn resize(&mut self, columns: u16, rows: u16) -> Result<()> {
        self.master.resize(size(columns, rows))?;
        // Only the main thread parses output, after updating the screen size.
        self.parser.screen_mut().set_size(rows, columns);
        Ok(())
    }

    pub fn termios(&self) -> Result<nix::sys::termios::Termios> {
        self.master.get_termios().context("read PTY termios")
    }

    pub fn wait_for(&mut self, step: &str, predicate: impl Fn(&Self) -> bool) -> Result<()> {
        while !predicate(self) {
            self.pump().with_context(|| format!("waiting for {step}"))?;
            ensure!(!self.eof || predicate(self), "PTY closed before {step}");
        }
        Ok(())
    }

    pub fn wait_output(&mut self, offset: usize, expected: &[u8]) -> Result<()> {
        ensure!(!expected.is_empty(), "empty output predicate");
        self.wait_for(&format!("output {expected:?}"), |host| {
            host.raw[offset..]
                .windows(expected.len())
                .any(|bytes| bytes == expected)
        })
    }

    pub fn finish(&mut self) -> Result<()> {
        while !self.eof || self.child.status.is_none() {
            self.pump()?;
        }
        self.join_reader()?;
        self.child.group_finished = true;
        let status = self.child.status.as_ref().context("missing child status")?;
        ensure!(status.success(), "PTY child exited: {status}");
        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<()> {
        let child = self.child.terminate();
        self.stop.store(true, Ordering::Relaxed);
        let reader = self.join_reader();
        let mut output = Ok(());
        for message in self.receiver.try_iter() {
            match message {
                Message::Output(bytes) => {
                    self.raw.extend_from_slice(&bytes);
                    self.parser.process(&bytes);
                }
                Message::End(result) => output = result.context("read PTY output during cleanup"),
            }
        }
        child.and(reader).and(output)
    }

    pub fn diagnostics(&self) -> String {
        let screen = self.parser.screen();
        let (rows, columns) = screen.size();
        let mut cells = Vec::new();
        for row in 0..rows {
            let line: Vec<_> = (0..columns)
                .map(|column| screen.cell(row, column).unwrap().contents())
                .collect();
            cells.push(format!("{row}: {line:?}"));
        }
        format!(
            "cursor={:?}, status={:?}, eof={}\nraw={:?}\ncells:\n{}",
            screen.cursor_position(),
            self.child.status,
            self.eof,
            self.raw,
            cells.join("\n")
        )
    }

    fn pump(&mut self) -> Result<()> {
        ensure!(
            !self.interrupted.load(Ordering::Relaxed),
            "host interrupted"
        );
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        ensure!(!remaining.is_zero(), "PTY deadline expired");
        match self
            .receiver
            .recv_timeout(remaining.min(Duration::from_millis(20)))
        {
            Ok(Message::Output(bytes)) => {
                self.raw.extend_from_slice(&bytes);
                self.parser.process(&bytes);
                for response in std::mem::take(&mut self.parser.callbacks_mut().replies) {
                    self.send(&response)?;
                }
            }
            Ok(Message::End(result)) => {
                self.eof = true;
                result.context("read PTY output")?;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) if self.eof => {
                std::thread::sleep(remaining.min(Duration::from_millis(5)));
            }
            Err(RecvTimeoutError::Disconnected) => bail!("PTY reader ended without a result"),
        }
        self.child.poll()
    }

    fn join_reader(&mut self) -> Result<()> {
        if let Some(reader) = self.reader.take() {
            reader
                .join()
                .map_err(|_| anyhow::anyhow!("PTY reader panicked"))?;
        }
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("PTY cleanup: {error:#}");
        }
    }
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}
