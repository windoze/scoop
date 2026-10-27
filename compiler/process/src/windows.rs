use std::collections::BTreeMap;
use std::io;
use std::os::windows::io::{AsHandle, AsRawHandle, OwnedHandle};
use std::process::{Child, Command, ExitStatus};
use std::sync::{
    Mutex,
    atomic::{AtomicI32, Ordering},
};

use windows_sys::Win32::Foundation::STATUS_CONTROL_C_EXIT;
use windows_sys::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
};
use windows_sys::Win32::System::Threading::{INFINITE, TerminateProcess, WaitForSingleObject};

static CHILDREN: Mutex<BTreeMap<u32, OwnedHandle>> = Mutex::new(BTreeMap::new());
static INTERRUPTED: AtomicI32 = AtomicI32::new(0);
static INITIALIZED: Mutex<bool> = Mutex::new(false);

pub(super) fn initialize() -> io::Result<()> {
    let mut initialized = INITIALIZED
        .lock()
        .map_err(|_| io::Error::other("console handler initialization lock poisoned"))?;
    if !*initialized {
        // The handler has static lifetime and Windows calls it on a new thread.
        if unsafe { SetConsoleCtrlHandler(Some(control_handler), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        *initialized = true;
    }
    Ok(())
}

unsafe extern "system" fn control_handler(event: u32) -> i32 {
    if !matches!(event, CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT) {
        return 0;
    }
    INTERRUPTED.store(STATUS_CONTROL_C_EXIT, Ordering::SeqCst);
    let Ok(children) = CHILDREN.lock() else {
        return 0;
    };
    for child in children.values() {
        // Owned handles keep the original process identity, including after exit.
        unsafe { TerminateProcess(child.as_raw_handle(), STATUS_CONTROL_C_EXIT as u32) };
    }
    if event == CTRL_CLOSE_EVENT {
        // Windows terminates this process when a close handler returns; finish
        // waiting here instead of relying on the main thread to run again.
        for child in children.values() {
            unsafe { WaitForSingleObject(child.as_raw_handle(), INFINITE) };
        }
    }
    1
}

pub(super) fn interrupted() -> Option<i32> {
    let status = INTERRUPTED.load(Ordering::SeqCst);
    (status != 0).then_some(status)
}

pub(super) fn spawn(command: &mut Command) -> io::Result<Child> {
    let mut children = CHILDREN
        .lock()
        .map_err(|_| io::Error::other("child process lock poisoned"))?;
    super::check_interrupted()?;
    let mut child = command.spawn()?;
    match child.as_handle().try_clone_to_owned() {
        Ok(handle) => {
            children.insert(child.id(), handle);
            Ok(child)
        }
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(error)
        }
    }
}

pub(super) fn try_wait(child: &mut Child, registered: &mut bool) -> io::Result<Option<ExitStatus>> {
    if !*registered {
        return child.try_wait();
    }
    let mut children = CHILDREN
        .lock()
        .map_err(|_| io::Error::other("child process lock poisoned"))?;
    let status = child.try_wait()?;
    if status.is_some() {
        children.remove(&child.id());
        *registered = false;
    }
    Ok(status)
}

#[cfg(test)]
#[path = "windows/tests.rs"]
mod tests;
