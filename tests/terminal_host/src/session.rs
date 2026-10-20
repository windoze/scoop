use anyhow::{Result, ensure};
use terminal_host::{Terminal, same_terminal_settings};

pub fn check(host: &mut Terminal) -> Result<()> {
    let mut offset = 0;
    for (stage, raw) in [
        ("raw-outer", true),
        ("raw-inner", true),
        ("raw-after-inner", true),
        ("restored", false),
        ("raw-throw", true),
        ("after-throw", false),
        ("raw-close", true),
        ("closed", false),
        ("redirected", false),
        ("redirected-raw", true),
    ] {
        host.wait_output(offset, format!("\x1b]0;{stage}\x07").as_bytes())?;
        let mut expected = host.initial_termios.clone();
        if raw {
            nix::sys::termios::cfmakeraw(&mut expected);
        }
        let actual = host.termios()?;
        ensure!(
            same_terminal_settings(&actual, &expected),
            "termios mismatch at {stage}: actual={actual:?}, expected={expected:?}"
        );
        offset = host.raw.len();
        if stage == "closed" {
            host.resize(26, 11)?;
        }
        host.send(b"\n")?;
    }
    host.wait_output(offset, b"\x1b]0;finished\x07")?;
    ensure!(
        same_terminal_settings(&host.termios()?, &host.initial_termios),
        "final termios restoration"
    );
    Ok(())
}
