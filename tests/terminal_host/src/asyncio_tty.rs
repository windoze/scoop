use anyhow::{Result, ensure};
use terminal_host::{Terminal, same_terminal_settings};

pub fn check(host: &mut Terminal) -> Result<()> {
    let mut offset = host.wait_output(0, b"\x1b]0;tty-tick\x07")?;
    host.send(b"t")?;
    offset = host.wait_output(offset, b"\x1b]0;tty-after-cancel\x07")?;
    host.send(b"c")?;
    offset = host.wait_output(offset, b"\x1b]0;tty-resize\x07")?;
    host.resize(35, 15)?;
    host.send(b"k")?;
    for stage in ["tty-restored", "tty-cancel-restored", "tty-loop-closed"] {
        offset = host.wait_output(offset, format!("\x1b]0;{stage}\x07").as_bytes())?;
        ensure!(
            same_terminal_settings(&host.termios()?, &host.initial_termios),
            "TTY settings were not restored at {stage}"
        );
        host.send(b"\n")?;
    }
    offset = host.wait_output(offset, b"\x1b]0;tty-eof\x07")?;
    host.send(b"\x04")?;
    host.wait_output(offset, b"\x1b]0;tty-finished\x07")?;
    ensure!(
        same_terminal_settings(&host.termios()?, &host.initial_termios),
        "TTY final settings"
    );
    Ok(())
}
