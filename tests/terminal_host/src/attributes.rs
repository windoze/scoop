use nix::sys::termios::{LocalFlags, Termios, cfgetispeed, cfgetospeed};

pub fn same_terminal_settings(actual: &Termios, expected: &Termios) -> bool {
    // Darwin sets PENDIN when ICANON is restored and clears it when input is read.
    // Compare semantic settings, not the wrapper's cached libc representation.
    let same = actual.input_flags == expected.input_flags
        && actual.output_flags == expected.output_flags
        && actual.control_flags == expected.control_flags
        && (actual.local_flags & !LocalFlags::PENDIN)
            == (expected.local_flags & !LocalFlags::PENDIN)
        && actual.control_chars == expected.control_chars
        && cfgetispeed(actual) == cfgetispeed(expected)
        && cfgetospeed(actual) == cfgetospeed(expected);
    #[cfg(target_os = "linux")]
    let same = same && actual.line_discipline == expected.line_discipline;
    same
}
