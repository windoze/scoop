use std::ffi::{OsStr, OsString};
use std::os::windows::{ffi::OsStrExt, io::FromRawHandle, process::ExitStatusExt};
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::Console::{
    GetConsoleWindow, GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, EXTENDED_STARTUPINFO_PRESENT,
    GetCurrentProcess, GetExitCodeProcess, PROCESS_INFORMATION, STARTF_USESTDHANDLES,
    STARTUPINFOEXW, STARTUPINFOW,
};

use super::*;

#[path = "program/handles.rs"]
mod handles;

pub(crate) fn run(
    executable: &Path,
    argv0: &Path,
    args: &[OsString],
    cwd: &Path,
) -> io::Result<ExitStatus> {
    let executable = wide(executable.as_os_str())?;
    let cwd = wide(cwd.as_os_str())?;
    let mut command = command_line(argv0.as_os_str(), args)?;
    let handles = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]
        .map(inherited_handle)
        .into_iter()
        .collect::<io::Result<Vec<_>>>()?;
    let raw = |index: usize| {
        handles[index]
            .as_ref()
            .map_or(null_mut(), AsRawHandle::as_raw_handle)
    };
    let selected = [raw(0), raw(1), raw(2)]
        .into_iter()
        .filter(|handle| !handle.is_null())
        .collect::<Vec<_>>();
    let mut attributes = handles::Attributes::new(&selected)?;
    let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = raw(0);
    startup.StartupInfo.hStdOutput = raw(1);
    startup.StartupInfo.hStdError = raw(2);
    let mut information: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let mut flags = CREATE_UNICODE_ENVIRONMENT;
    if let Some(attributes) = &mut attributes {
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.lpAttributeList = attributes.as_mut_ptr();
        flags |= EXTENDED_STARTUPINFO_PRESENT;
    }
    if unsafe { GetConsoleWindow() }.is_null() {
        flags |= CREATE_NO_WINDOW;
    }
    let mut children = CHILDREN
        .lock()
        .map_err(|_| io::Error::other("child process lock poisoned"))?;
    crate::check_interrupted()?;
    // lpApplicationName selects the stable cached image; lpCommandLine retains
    // the user's published path as argv[0]. No shell or execution copy is used.
    if unsafe {
        CreateProcessW(
            executable.as_ptr(),
            command.as_mut_ptr(),
            null(),
            null(),
            i32::from(!selected.is_empty()),
            flags,
            null(),
            cwd.as_ptr(),
            &startup.StartupInfo,
            &mut information,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let process = unsafe { OwnedHandle::from_raw_handle(information.hProcess) };
    drop(unsafe { OwnedHandle::from_raw_handle(information.hThread) });
    match process.try_clone() {
        Ok(handle) => children.insert(information.dwProcessId, handle),
        Err(error) => {
            unsafe {
                TerminateProcess(process.as_raw_handle(), 1);
                WaitForSingleObject(process.as_raw_handle(), INFINITE);
            }
            return Err(error);
        }
    };
    drop(children);
    ProgramChild {
        process,
        id: information.dwProcessId,
        completed: false,
    }
    .wait()
}

struct ProgramChild {
    process: OwnedHandle,
    id: u32,
    completed: bool,
}

impl ProgramChild {
    fn wait(mut self) -> io::Result<ExitStatus> {
        let handle = self.process.as_raw_handle();
        if unsafe { WaitForSingleObject(handle, INFINITE) } != WAIT_OBJECT_0 {
            return Err(io::Error::last_os_error());
        }
        self.completed = true;
        let mut code = 0;
        if unsafe { GetExitCodeProcess(handle, &mut code) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(ExitStatus::from_raw(code))
    }
}

impl Drop for ProgramChild {
    fn drop(&mut self) {
        if !self.completed {
            unsafe {
                TerminateProcess(self.process.as_raw_handle(), 1);
                WaitForSingleObject(self.process.as_raw_handle(), INFINITE);
            }
        }
        if let Ok(mut children) = CHILDREN.lock() {
            children.remove(&self.id);
        }
    }
}

fn inherited_handle(which: u32) -> io::Result<Option<OwnedHandle>> {
    let source = unsafe { GetStdHandle(which) };
    if source.is_null() || source == INVALID_HANDLE_VALUE {
        return Ok(None);
    }
    let mut handle: HANDLE = null_mut();
    let current = unsafe { GetCurrentProcess() };
    if unsafe {
        DuplicateHandle(
            current,
            source,
            current,
            &mut handle,
            0,
            1,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(Some(unsafe { OwnedHandle::from_raw_handle(handle) }))
}

fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut units = value.encode_wide().collect::<Vec<_>>();
    if units.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NUL in process argument",
        ));
    }
    units.push(0);
    Ok(units)
}

fn command_line(argv0: &OsStr, args: &[OsString]) -> io::Result<Vec<u16>> {
    let program = wide(argv0)?;
    if program.len() == 1 || program.contains(&u16::from(b'"')) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid program argv[0]",
        ));
    }
    // The first CRT argument treats backslashes literally inside quotes.
    let mut line = vec![u16::from(b'"')];
    line.extend_from_slice(&program[..program.len() - 1]);
    line.push(u16::from(b'"'));
    for argument in args {
        let argument = wide(argument)?;
        line.extend([u16::from(b' '), u16::from(b'"')]);
        let mut slashes = 0;
        for &unit in &argument[..argument.len() - 1] {
            if unit == u16::from(b'\\') {
                slashes += 1;
            } else {
                let count = if unit == u16::from(b'"') {
                    slashes * 2 + 1
                } else {
                    slashes
                };
                line.extend(std::iter::repeat_n(u16::from(b'\\'), count));
                line.push(unit);
                slashes = 0;
            }
        }
        line.extend(std::iter::repeat_n(u16::from(b'\\'), slashes * 2));
        line.push(u16::from(b'"'));
    }
    line.push(0);
    Ok(line)
}
