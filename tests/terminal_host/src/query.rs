use anyhow::{Result, ensure};
use terminal_host::{Terminal, same_terminal_settings};

fn stage(host: &mut Terminal, offset: &mut usize, name: &str, request: &[u8]) -> Result<()> {
    *offset = host.wait_output(*offset, format!("\x1b]0;query-{name}\x07").as_bytes())?;
    *offset = host.wait_output(*offset, request)?;
    Ok(())
}

pub fn check(host: &mut Terminal, asynchronous: bool) -> Result<()> {
    host.parser.callbacks_mut().silent = true;
    let mut offset = 0;
    stage(host, &mut offset, "cursor", b"\x1b[6n")?;
    ensure!(host.parser.screen().cursor_position() == (3, 8));
    host.send(b"a\x1b[I\x1b[4;")?;
    host.send(b"9Rb")?;
    stage(host, &mut offset, "primary", b"\x1b[c")?;
    host.send(b"\x1b[?1;2c")?;
    stage(host, &mut offset, "secondary", b"\x1b[>c")?;
    host.send(b"\x1b[?1;2c\x1b[>0;100;0c")?;
    stage(host, &mut offset, "kitty-zero", b"\x1b[?u\x1b[c")?;
    host.send(b"m\x1b[?0u\x1b[O\x1b[?1;2cn")?;
    stage(host, &mut offset, "kitty-no", b"\x1b[?u\x1b[c")?;
    host.send(b"\x1b[?1;2c")?;
    stage(host, &mut offset, "kitty-yes", b"\x1b[?u\x1b[c")?;
    host.send(b"\x1b[?15u\x1b[?1;2c")?;
    stage(host, &mut offset, "timeout", b"\x1b[6n")?;
    stage(host, &mut offset, "zero", b"\x1b[6n")?;
    stage(host, &mut offset, "kitty-incomplete", b"\x1b[?u\x1b[c")?;
    host.send(b"\x1b[?15u")?;
    stage(host, &mut offset, "invalid", b"\x1b[6n")?;
    host.send(b"q\x1b[0;9Rr")?;
    offset = host.wait_output(offset, b"\x1b]0;query-late-queued\x07")?;
    host.send(b"x\x1b[9;10R\x1b[?15u\x1b[?1;2c")?;
    stage(host, &mut offset, "fresh-cursor", b"\x1b[6n")?;
    host.send(b"\x1b[2;3R")?;
    if asynchronous {
        stage(host, &mut offset, "cancel", b"\x1b[6n")?;
        host.send(b"u\x1b[I")?;
        offset = host.wait_output(offset, b"\x1b]0;query-after-cancel\x07")?;
        ensure!(
            same_terminal_settings(&host.termios()?, &host.initial_termios),
            "cancelled query did not restore terminal settings"
        );
        host.send(b"\n")?;
    }
    host.wait_output(offset, b"\x1b]0;query-finished\x07")?;
    ensure!(
        same_terminal_settings(&host.termios()?, &host.initial_termios),
        "query session did not restore terminal settings"
    );
    Ok(())
}
