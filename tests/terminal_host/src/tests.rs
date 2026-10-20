use crate::{Protocol, Terminal};
use portable_pty::CommandBuilder;

#[test]
fn replies_observe_query_position_at_every_split() {
    let bytes = b"\x1b[3;7H\x1b[6n\x1b[1;1H\x1b[5n\x1b[c\x1b[>c\x1b[?u";
    for cut in 0..=bytes.len() {
        let callbacks = Protocol {
            kitty_flags: Some(0),
            ..Protocol::default()
        };
        let mut parser = vt100::Parser::new_with_callbacks(8, 20, 0, callbacks);
        parser.process(&bytes[..cut]);
        parser.process(&bytes[cut..]);
        assert_eq!(
            parser.callbacks().replies,
            [
                b"\x1b[3;7R".to_vec(),
                b"\x1b[0n".to_vec(),
                b"\x1b[?1;2c".to_vec(),
                b"\x1b[>0;100;0c".to_vec(),
                b"\x1b[?0u".to_vec()
            ],
            "split {cut}"
        );
        assert_eq!(parser.screen().cursor_position(), (0, 0));
    }
}

#[test]
fn osc_payload_is_not_a_query_and_unsupported_kitty_stays_silent() {
    let mut parser = vt100::Parser::new_with_callbacks(8, 20, 0, Protocol::default());
    parser.process(b"\x1b]52;c;WzZu\x07\x1b[?u");
    assert!(parser.callbacks().replies.is_empty());
    assert_eq!(parser.callbacks().requests, ["kitty"]);
    assert_eq!(
        parser.callbacks().clipboard,
        [(b"c".to_vec(), b"WzZu".to_vec())]
    );
}

#[test]
fn real_pty_drains_output_and_preserves_exact_cells() {
    let mut command = CommandBuilder::new("/bin/sh");
    command.args(["-c", "printf '\\033[2;3H A  '; printf '\\033[4;1Hdone'"]);
    let mut host = Terminal::spawn(command, 12, 5).unwrap();
    host.finish().unwrap();
    assert!(host.raw.ends_with(b"done"));
    assert_eq!(host.parser.screen().cell(1, 2).unwrap().contents(), " ");
    assert_eq!(host.parser.screen().cell(1, 3).unwrap().contents(), "A");
    assert_eq!(host.parser.screen().cell(1, 5).unwrap().contents(), " ");
    assert_eq!(host.parser.screen().cell(1, 6).unwrap().contents(), "");
    assert_eq!(host.parser.screen().cell(4, 11).unwrap().contents(), "");
    assert_eq!(host.parser.screen().cursor_position(), (3, 4));
}

#[test]
fn real_pty_interaction_resize_and_failure_cleanup() {
    let mut command = CommandBuilder::new("/bin/sh");
    command.args([
        "-c",
        "printf ready; read value; printf 'reply:%s' \"$value\"; read value",
    ]);
    let mut host = Terminal::spawn(command, 12, 5).unwrap();
    host.wait_output(0, b"ready").unwrap();
    host.resize(18, 7).unwrap();
    host.send(b"answer\n").unwrap();
    host.wait_output(0, b"reply:answer").unwrap();
    assert_eq!(host.parser.screen().size(), (7, 18));
    host.cleanup().unwrap();
    host.cleanup().unwrap();
}

#[test]
fn real_pty_exit_before_expected_output_is_reaped() {
    let mut command = CommandBuilder::new("/bin/sh");
    command.args(["-c", "exit 17"]);
    let mut host = Terminal::spawn(command, 12, 5).unwrap();
    assert!(host.wait_output(0, b"missing").is_err());
    host.cleanup().unwrap();
}
