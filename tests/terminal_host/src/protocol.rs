#[derive(Default)]
pub struct Protocol {
    pub replies: Vec<Vec<u8>>,
    pub title: Vec<u8>,
    pub clipboard: Vec<(Vec<u8>, Vec<u8>)>,
    pub requests: Vec<String>,
    pub silent: bool,
    pub kitty_flags: Option<u16>,
}

impl vt100::Callbacks for Protocol {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.title = title.to_vec();
    }

    fn copy_to_clipboard(&mut self, _: &mut vt100::Screen, selection: &[u8], data: &[u8]) {
        self.clipboard.push((selection.to_vec(), data.to_vec()));
    }

    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        first: Option<u8>,
        second: Option<u8>,
        params: &[&[u16]],
        end: char,
    ) {
        if second.is_some() || params.iter().any(|param| param.len() != 1) {
            return;
        }
        let numbers: Vec<u16> = params.iter().map(|param| param[0]).collect();
        let (name, response) = match (first, numbers.as_slice(), end) {
            (None, [6], 'n') => {
                let (row, column) = screen.cursor_position();
                ("cursor", Some(format!("\x1b[{};{}R", row + 1, column + 1)))
            }
            (None, [5], 'n') => ("status", Some("\x1b[0n".into())),
            (None, [] | [0], 'c') => ("primary", Some("\x1b[?1;2c".into())),
            (Some(b'>'), [] | [0], 'c') => ("secondary", Some("\x1b[>0;100;0c".into())),
            (Some(b'?'), [] | [0], 'u') => (
                "kitty",
                self.kitty_flags.map(|flags| format!("\x1b[?{flags}u")),
            ),
            _ => return,
        };
        self.requests.push(name.into());
        if !self.silent
            && let Some(response) = response
        {
            self.replies.push(response.into_bytes());
        }
    }
}
