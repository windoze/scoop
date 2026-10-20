use anyhow::{Result, ensure};
use terminal_host::{Terminal, same_terminal_settings};

pub fn check(host: &mut Terminal) -> Result<()> {
    let mut offset = 0;
    for (stage, bytes) in [
        ("async-prefix", b"\xc3".as_slice()),
        ("async-tail", b"\xa9"),
        ("async-pair", b"ab"),
        ("async-escape", b"\x1b"),
        ("async-tick", b"t"),
        (
            "async-events",
            b"\x1b[I\x1b[<0;7;3M\x1b[200~hello\x1b[201~\x1b[97;5:3u",
        ),
        ("async-invalid", b"\xffz"),
    ] {
        offset = host.wait_output(offset, format!("\x1b]0;{stage}\x07").as_bytes())?;
        host.send(bytes)?;
    }
    offset = host.wait_output(offset, b"\x1b]0;async-resize\x07")?;
    host.resize(35, 15)?;
    host.send(b"r")?;
    for stage in ["async-restored", "async-cancel-restored", "async-eof"] {
        offset = host.wait_output(offset, format!("\x1b]0;{stage}\x07").as_bytes())?;
        ensure!(
            same_terminal_settings(&host.termios()?, &host.initial_termios),
            "terminal mode was not restored at {stage}"
        );
        host.send(if stage == "async-eof" { b"\x04" } else { b"\n" })?;
    }
    host.wait_output(offset, b"\x1b]0;async-finished\x07")?;
    ensure!(
        same_terminal_settings(&host.termios()?, &host.initial_termios),
        "async terminal final settings"
    );
    Ok(())
}
