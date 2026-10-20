use anyhow::{Result, ensure};
use terminal_host::{Terminal, same_terminal_settings};

fn stage(host: &mut Terminal, offset: &mut usize, name: &str) -> Result<()> {
    *offset = host.wait_output(*offset, format!("\x1b]0;{name}\x07").as_bytes())?;
    Ok(())
}

pub fn check(host: &mut Terminal) -> Result<()> {
    let mut offset = 0;
    stage(host, &mut offset, "utf8-prefix")?;
    host.send(b"\xc3")?;
    stage(host, &mut offset, "utf8-tail")?;
    host.send(b"\xa9")?;
    stage(host, &mut offset, "pair")?;
    host.send(b"ab")?;
    stage(host, &mut offset, "escape")?;
    host.send(b"\x1b")?;
    stage(host, &mut offset, "escape-pending")?;
    stage(host, &mut offset, "escape-done")?;
    stage(host, &mut offset, "events")?;
    host.send(b"\x1b[I\x1b[<0;7;3M\x1b[200~hello\x1b[201~\x1b[97;5:3u")?;
    stage(host, &mut offset, "invalid")?;
    host.send(b"\xffz")?;
    stage(host, &mut offset, "resize-0")?;
    host.resize(30, 12)?;
    host.send(b"k")?;
    for mode in 1..=4 {
        stage(host, &mut offset, &format!("resize-{mode}"))?;
        host.resize(30 + mode, 12 + mode)?;
    }
    stage(host, &mut offset, "eof-prefix")?;
    host.send(b"\x1b\x04")?;
    stage(host, &mut offset, "eof-next")?;
    host.send(b"\x04")?;
    stage(host, &mut offset, "input-finished")?;
    ensure!(
        same_terminal_settings(&host.termios()?, &host.initial_termios),
        "input session did not restore terminal settings"
    );
    Ok(())
}
