use anyhow::{Result, ensure};
use terminal_host::Terminal;

pub fn check(host: &mut Terminal) -> Result<()> {
    host.wait_output(0, b"\x1b]0;clipboard-done\x07")?;
    let expected = [
        (b"c".to_vec(), b"Zm9v".to_vec()),
        (b"pc".to_vec(), b"AAEC/w==".to_vec()),
        (b"p".to_vec(), b"5LitChtd".to_vec()),
        (Vec::new(), Vec::new()),
    ];
    ensure!(
        host.parser.callbacks().clipboard == expected,
        "OSC 52 contents"
    );
    let screen = host.parser.screen();
    for (column, character) in ["k", "e", "e", "p"].iter().enumerate() {
        ensure!(
            screen.cell(2, column as u16 + 2).unwrap().contents() == *character,
            "clipboard changed the screen"
        );
    }
    ensure!(
        screen.cursor_position() == (2, 6),
        "clipboard moved the cursor"
    );
    for (selection, content) in expected {
        let frame = format!(
            "\x1b]52;{};{}\x1b\\",
            String::from_utf8(selection)?,
            String::from_utf8(content)?
        );
        ensure!(
            host.raw
                .windows(frame.len())
                .any(|bytes| bytes == frame.as_bytes()),
            "OSC 52 must use ST"
        );
    }
    Ok(())
}
