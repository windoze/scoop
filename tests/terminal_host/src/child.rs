use anyhow::{Context, Result};
use portable_pty::{Child, ExitStatus};

pub(crate) struct OwnedChild {
    pub child: Box<dyn Child + Send + Sync>,
    pub status: Option<ExitStatus>,
    pub group_finished: bool,
}

impl OwnedChild {
    pub fn poll(&mut self) -> Result<()> {
        if self.status.is_none() {
            self.status = self.child.try_wait().context("poll PTY child")?;
        }
        Ok(())
    }

    pub fn terminate(&mut self) -> Result<()> {
        // portable-pty starts a new session; the outer runner cannot kill it.
        // A descendant can retain the PTY after the leader exits.
        if !self.group_finished {
            let pid = self.child.process_id().context("PTY child has no PID")? as libc::pid_t;
            if unsafe { libc::kill(-pid, libc::SIGKILL) } == -1 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error).context("kill PTY process group");
                }
            }
            self.group_finished = true;
        }
        if self.status.is_none() {
            self.status = Some(self.child.wait().context("reap PTY child")?);
        }
        Ok(())
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Err(error) = self.terminate() {
            eprintln!("PTY cleanup: {error:#}");
        }
    }
}
