use pretty::{Doc, RcDoc, Render, RenderAnnotated};

pub(crate) type Document = RcDoc<'static, ()>;

pub(crate) fn token(text: &str) -> Document {
    let parts = text.split('\n').map(|part| {
        Document::from(Doc::RenderLen(
            part.chars().count(),
            Doc::OwnedText(part.into()).into(),
        ))
    });
    let document = Document::intersperse(parts, Document::hardline());
    Document::nesting(move |indent| document.clone().nest(-(indent as isize))).annotate(())
}

pub(crate) fn render(document: Document, crlf: bool) -> String {
    let mut writer = Writer {
        text: String::new(),
        pending: String::new(),
        raw: 0,
        crlf,
    };
    document
        .append(Document::hardline())
        .render_raw(100, &mut writer)
        .expect("string rendering succeeds");
    writer.text
}

struct Writer {
    text: String,
    pending: String,
    raw: usize,
    crlf: bool,
}

impl Render for Writer {
    type Error = std::fmt::Error;

    fn write_str(&mut self, text: &str) -> Result<usize, Self::Error> {
        if self.raw != 0 {
            self.text.push_str(&std::mem::take(&mut self.pending));
            self.text.push_str(text);
        } else {
            for character in text.chars() {
                if character == '\n' {
                    self.pending.clear();
                    self.text.push_str(if self.crlf { "\r\n" } else { "\n" });
                } else {
                    self.pending.push(character);
                }
            }
        }
        Ok(text.len())
    }

    fn fail_doc(&self) -> Self::Error {
        std::fmt::Error
    }
}

impl RenderAnnotated<'_, ()> for Writer {
    fn push_annotation(&mut self, _: &()) -> Result<(), Self::Error> {
        self.raw += 1;
        Ok(())
    }

    fn pop_annotation(&mut self) -> Result<(), Self::Error> {
        self.raw -= 1;
        Ok(())
    }
}
