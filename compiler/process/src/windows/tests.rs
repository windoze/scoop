use std::os::windows::process::CommandExt as _;
use std::time::Duration;

use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

use super::*;
use crate::CommandExt as _;

const EXIT_CODE: i32 = 0x81234567u32 as i32;
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn argument_values() -> Vec<std::ffi::OsString> {
    [
        "",
        "two words",
        "引数 😀",
        r#"embedded\"quote"#,
        r"trailing\\",
        "--flag",
    ]
    .into_iter()
    .map(Into::into)
    .collect()
}

#[test]
#[ignore = "child process invoked by the Windows program argument test"]
fn program_arguments() {
    let args = std::env::args_os().collect::<Vec<_>>();
    assert_eq!(&args[5..args.len() - 1], argument_values());
    let published = std::path::PathBuf::from(args.last().unwrap());
    assert_eq!(args[0], published.as_os_str());
    assert_eq!(
        std::env::current_dir().unwrap(),
        published.parent().unwrap()
    );
    assert_ne!(std::env::current_exe().unwrap(), published);
}

#[test]
fn selected_program_preserves_argv0_unicode_quotes_and_cwd() {
    let _lock = TEST_LOCK.lock().unwrap();
    let root = tempfile::Builder::new()
        .prefix("scoop 目录 ")
        .tempdir()
        .unwrap();
    let executable = std::env::current_exe().unwrap();
    let output = root.path().join("published 名称.exe");
    std::fs::copy(&executable, &output).unwrap();
    let mut args = [
        "--ignored",
        "--exact",
        "signals::tests::program_arguments",
        "--",
    ]
    .into_iter()
    .map(Into::into)
    .collect::<Vec<std::ffi::OsString>>();
    args.extend(argument_values());
    args.push(output.as_os_str().to_owned());
    assert!(
        crate::run_program(&executable, &output, &args, root.path())
            .unwrap()
            .success()
    );
    assert!(CHILDREN.lock().unwrap().is_empty());
    assert_eq!(
        crate::run_program(&executable, &output, &["bad\0argument".into()], root.path())
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
}

fn helper(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--ignored", "--exact", "signals::tests::child_process"])
        .env("SCOOP_PROCESS_TEST_MODE", mode)
        .creation_flags(CREATE_NO_WINDOW);
    command
}

#[test]
#[ignore = "child process invoked by the Windows process test"]
fn child_process() {
    match std::env::var("SCOOP_PROCESS_TEST_MODE").unwrap().as_str() {
        "exit" => std::process::exit(EXIT_CODE),
        "forward" => {
            let output = helper("exit").scoop_output().unwrap();
            crate::exit_with_status(output.status);
        }
        "sleep" => std::thread::sleep(Duration::from_secs(120)),
        "interrupt-c" => interrupt(CTRL_C_EVENT),
        "interrupt-break" => interrupt(CTRL_BREAK_EVENT),
        "interrupt-close" => interrupt(CTRL_CLOSE_EVENT),
        mode => panic!("unknown child mode: {mode}"),
    }
}

#[test]
fn owned_children_are_reaped_and_windows_status_is_preserved() {
    let _lock = TEST_LOCK.lock().unwrap();
    let output = helper("forward").scoop_output().unwrap();
    assert_eq!(output.status.code(), Some(EXIT_CODE));
    assert!(CHILDREN.lock().unwrap().is_empty());

    let child = helper("sleep").scoop_spawn().unwrap();
    let handle = child.as_handle().try_clone_to_owned().unwrap();
    drop(child);
    assert_eq!(
        unsafe { WaitForSingleObject(handle.as_raw_handle(), 10_000) },
        WAIT_OBJECT_0
    );
    assert!(CHILDREN.lock().unwrap().is_empty());

    assert_eq!(unsafe { control_handler(u32::MAX) }, 0);
    assert_eq!(interrupted(), None);
    for mode in ["interrupt-c", "interrupt-break", "interrupt-close"] {
        let output = helper(mode).scoop_output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(STATUS_CONTROL_C_EXIT),
            "{output:?}"
        );
        assert!(CHILDREN.lock().unwrap().is_empty());
    }
    assert_eq!(interrupted(), None);
}

fn interrupt(event: u32) {
    let mut first = helper("sleep").scoop_spawn().unwrap();
    let mut second = helper("sleep").scoop_spawn().unwrap();
    assert_eq!(unsafe { control_handler(event) }, 1);
    assert_eq!(first.wait().unwrap().code(), Some(STATUS_CONTROL_C_EXIT));
    assert_eq!(second.wait().unwrap().code(), Some(STATUS_CONTROL_C_EXIT));
    assert!(CHILDREN.lock().unwrap().is_empty());
    assert_eq!(interrupted(), Some(STATUS_CONTROL_C_EXIT));
    let error = helper("exit").scoop_spawn().err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    crate::finish_interruption();
    panic!("interruption must exit the child");
}
