use std::collections::HashSet;
use std::io;
use std::process::{Child, Command, ExitStatus};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicI32, Ordering},
};

use rustix::process::{Pid, Signal, kill_process};
use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;

struct ActiveChildren {
    pids: Mutex<HashSet<Pid>>,
    interrupted: AtomicI32,
}
static CHILDREN: OnceLock<Arc<ActiveChildren>> = OnceLock::new();
static INITIALIZE: Mutex<()> = Mutex::new(());

pub(super) fn initialize() -> io::Result<()> {
    let _lock = INITIALIZE
        .lock()
        .map_err(|_| io::Error::other("signal initialization lock poisoned"))?;
    if CHILDREN.get().is_some() {
        return Ok(());
    }
    let mut signals = Signals::new([SIGHUP, SIGINT, SIGQUIT, SIGTERM])?;
    let children = Arc::new(ActiveChildren {
        pids: Mutex::new(HashSet::new()),
        interrupted: AtomicI32::new(0),
    });
    let relay = Arc::clone(&children);
    std::thread::Builder::new()
        .name("scoop-signals".to_owned())
        .spawn(move || {
            for signal in signals.forever() {
                relay.interrupted.store(signal, Ordering::SeqCst);
                if let Some(signal) = Signal::from_named_raw(signal)
                    && let Ok(pids) = relay.pids.lock()
                {
                    for pid in pids.iter() {
                        let _ = kill_process(*pid, signal);
                    }
                }
            }
        })?;
    CHILDREN
        .set(children)
        .map_err(|_| io::Error::other("signal handling already initialized"))
}

pub(super) fn interrupted() -> Option<i32> {
    CHILDREN
        .get()
        .map(|children| children.interrupted.load(Ordering::SeqCst))
        .filter(|signal| *signal != 0)
}

pub(super) fn spawn(command: &mut Command) -> io::Result<Child> {
    let children = CHILDREN
        .get()
        .expect("process initialization precedes spawn");
    let mut pids = children
        .pids
        .lock()
        .map_err(|_| io::Error::other("child process lock poisoned"))?;
    super::check_interrupted()?;
    let child = command.spawn()?;
    pids.insert(Pid::from_child(&child));
    Ok(child)
}

pub(super) fn try_wait(child: &mut Child, registered: &mut bool) -> io::Result<Option<ExitStatus>> {
    if !*registered {
        return child.try_wait();
    }
    let children = CHILDREN
        .get()
        .expect("a registered child has signal handling");
    let mut pids = children
        .pids
        .lock()
        .map_err(|_| io::Error::other("child process lock poisoned"))?;
    let status = child.try_wait()?;
    if status.is_some() {
        pids.remove(&Pid::from_child(child));
        *registered = false;
    }
    Ok(status)
}
