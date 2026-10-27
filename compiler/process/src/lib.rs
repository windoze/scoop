//! Scoped child-process I/O and interruption handling for compiler tools.
use std::io::{self, Read};
use std::ops::{Deref, DerefMut};
use std::process::{Child, Command, ExitStatus, Output, Stdio};

#[cfg(unix)]
mod signals;
#[cfg(windows)]
#[path = "windows.rs"]
mod signals;

mod depfile;
pub use depfile::parse_make_dependencies;

/// Run the selected image with a distinct argv[0], inherited I/O and environment.
#[cfg(windows)]
pub fn run_program(
    executable: &std::path::Path,
    argv0: &std::path::Path,
    args: &[std::ffi::OsString],
    cwd: &std::path::Path,
) -> io::Result<ExitStatus> {
    initialize()?;
    signals::program::run(executable, argv0, args, cwd)
}

pub fn initialize() -> io::Result<()> {
    #[cfg(any(unix, windows))]
    signals::initialize()?;
    Ok(())
}

pub fn interrupted_signal() -> Option<i32> {
    #[cfg(any(unix, windows))]
    {
        signals::interrupted()
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

pub fn check_interrupted() -> io::Result<()> {
    if interrupted_signal().is_some() {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "process interrupted",
        ))
    } else {
        Ok(())
    }
}

pub fn finish_interruption() {
    if let Some(signal) = interrupted_signal() {
        exit_with_signal(signal);
    }
}

pub fn exit_with_status(status: ExitStatus) -> ! {
    if let Some(code) = status.code() {
        std::process::exit(code);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            exit_with_signal(signal);
        }
    }
    std::process::exit(1)
}

fn exit_with_signal(signal: i32) -> ! {
    #[cfg(unix)]
    let _ = signal_hook::low_level::emulate_default_handler(signal);
    #[cfg(not(any(unix, windows)))]
    let _ = signal;
    #[cfg(windows)]
    std::process::exit(signal);
    #[cfg(not(windows))]
    std::process::exit(1)
}

pub trait CommandExt {
    fn scoop_spawn(&mut self) -> io::Result<ToolChild>;
    /// Capture stdout/stderr for a noninteractive compiler tool.
    fn scoop_output(&mut self) -> io::Result<Output>;
}

impl CommandExt for Command {
    fn scoop_spawn(&mut self) -> io::Result<ToolChild> {
        initialize()?;
        check_interrupted()?;
        #[cfg(any(unix, windows))]
        let child = signals::spawn(self)?;
        #[cfg(not(any(unix, windows)))]
        let child = self.spawn()?;
        Ok(ToolChild {
            child,
            registered: true,
        })
    }

    fn scoop_output(&mut self) -> io::Result<Output> {
        self.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .scoop_spawn()?
            .wait_with_output()
    }
}

pub struct ToolChild {
    child: Child,
    registered: bool,
}

impl Deref for ToolChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.child
    }
}
impl DerefMut for ToolChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child
    }
}

impl ToolChild {
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        #[cfg(any(unix, windows))]
        let status = signals::try_wait(&mut self.child, &mut self.registered)?;
        #[cfg(not(any(unix, windows)))]
        let status = self.child.try_wait()?;
        if status.is_some() {
            self.registered = false;
        }
        Ok(status)
    }

    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    pub fn wait_with_output(mut self) -> io::Result<Output> {
        drop(self.child.stdin.take());
        let stdout = self.child.stdout.take();
        let stderr = self.child.stderr.take();
        let stdout = std::thread::spawn(move || read_pipe(stdout));
        let stderr = std::thread::spawn(move || read_pipe(stderr));
        let status = self.wait()?;
        let stdout = stdout
            .join()
            .map_err(|_| io::Error::other("stdout reader panicked"))??;
        let stderr = stderr
            .join()
            .map_err(|_| io::Error::other("stderr reader panicked"))??;
        Ok(Output {
            status,
            stdout,
            stderr,
        })
    }
}

impl Drop for ToolChild {
    fn drop(&mut self) {
        if self.registered {
            let _ = self.child.kill();
            let _ = self.wait();
        }
    }
}

fn read_pipe(pipe: Option<impl Read>) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    if let Some(mut pipe) = pipe {
        pipe.read_to_end(&mut bytes)?;
    }
    Ok(bytes)
}
