use anyhow::{Result, ensure};
use terminal_host::Terminal;
use vt100::Color;

pub fn check(host: &mut Terminal) -> Result<()> {
    let initial = host.termios()?;
    host.wait_output(0, b"stage-one")?;
    let screen = host.parser.screen();
    ensure!(screen.size() == (8, 20), "initial dimensions");
    ensure!(
        screen.cell(1, 2).unwrap().contents() == "A",
        "absolute cursor"
    );
    let styled = screen.cell(1, 3).unwrap();
    ensure!(styled.contents() == "B" && styled.bold(), "styled cell");
    ensure!(styled.fgcolor() == Color::Rgb(12, 34, 56), "RGB foreground");
    ensure!(styled.bgcolor() == Color::Idx(4), "indexed background");
    ensure!(
        screen.cell(1, 4).unwrap().fgcolor() == Color::Default,
        "style reset"
    );
    ensure!(
        screen.cell(2, 2).unwrap().contents() == "界",
        "wide character"
    );
    ensure!(
        screen.cell(2, 3).unwrap().is_wide_continuation(),
        "wide continuation"
    );
    ensure!(
        screen.cell(2, 4).unwrap().contents() == "é",
        "Unicode column"
    );
    ensure!(screen.cell(3, 5).unwrap().contents() == "R", "saved cursor");
    ensure!(screen.hide_cursor(), "hidden cursor");
    ensure!(
        host.parser.callbacks().title == b"Scoop PTY",
        "window title"
    );

    let offset = host.raw.len();
    host.send(b"\n")?;
    host.wait_output(offset, b"stage-two")?;
    let screen = host.parser.screen();
    ensure!(screen.alternate_screen(), "alternate screen entered");
    ensure!(screen.cell(0, 0).unwrap().contents() == "S", "scroll up");
    ensure!(
        screen.cell(1, 0).unwrap().contents().is_empty(),
        "scrolled blank row"
    );
    ensure!(
        screen.cell(2, 2).unwrap().contents().is_empty(),
        "wide cell cleared"
    );
    ensure!(screen.cell(2, 3).unwrap().contents() == "X", "wide overlap");
    ensure!(
        !screen.cell(2, 3).unwrap().is_wide_continuation(),
        "overlap flag"
    );
    // vt100 0.16 does not implement DECAWM; assert its wire contract instead.
    let no_wrap = b"\x1b[?7l\x1b[4;20HYZ";
    ensure!(
        host.raw[offset..]
            .windows(no_wrap.len())
            .any(|bytes| bytes == no_wrap),
        "disable-wrap command"
    );
    ensure!(
        screen.cell(5, 4).unwrap().contents().is_empty(),
        "line erase"
    );
    ensure!(
        screen.cell(5, 5).unwrap().contents().is_empty(),
        "trailing blank cell"
    );

    let offset = host.raw.len();
    host.resize(24, 10)?;
    host.send(b"\n")?;
    host.wait_output(offset, b"stage-three")?;
    let screen = host.parser.screen();
    ensure!(!screen.alternate_screen(), "alternate screen left");
    ensure!(screen.size() == (10, 24), "resized screen");
    ensure!(
        screen.cell(1, 2).unwrap().contents() == "A",
        "primary screen restored"
    );
    ensure!(!screen.hide_cursor(), "cursor shown");
    ensure!(
        host.termios()? == initial,
        "output commands changed termios"
    );
    host.send(b"\n")
}
