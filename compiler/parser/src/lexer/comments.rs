//! Trivia and documentation share the ordinary lexer, including interpolation.

use super::Lexer;
use scoop_ast::{Diagnostic, DocComment, DocCommentKind, DocCommentTarget};

#[derive(Debug)]
pub(crate) struct LexicalDocComment {
    pub before_token: usize,
    pub comment: DocComment,
}

impl Lexer<'_> {
    pub(super) fn skip_trivia(&mut self, before_token: usize) -> Result<(), Diagnostic> {
        loop {
            match self.peek_char() {
                Some(' ' | '\t' | '\r') => self.pos += 1,
                Some('\n') => {
                    self.pos += 1;
                    self.newline_before = true;
                }
                Some('#') if self.pos == 0 && self.source.starts_with("#!") => {
                    self.skip_line_comment();
                }
                Some('/') if self.source[self.pos + 1..].starts_with('/') => {
                    let start = self.pos;
                    let rest = &self.source[start..];
                    let is_doc = rest.starts_with("///") && !rest.starts_with("////");
                    self.skip_line_comment();
                    if is_doc {
                        self.save_doc(start, before_token, DocCommentKind::Line);
                    }
                }
                Some('/') if self.source[self.pos + 1..].starts_with('*') => {
                    let start = self.pos;
                    let rest = &self.source[start..];
                    let is_doc = rest.starts_with("/**") && !rest.starts_with("/***");
                    self.skip_block_comment()?;
                    if is_doc {
                        self.save_doc(start, before_token, DocCommentKind::Block);
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(c) = self.peek_char() {
            if c == '\n' {
                break;
            }
            self.pos += c.len_utf8();
        }
    }

    fn save_doc(&mut self, start: usize, before_token: usize, kind: DocCommentKind) {
        self.documentation.push(LexicalDocComment {
            before_token,
            comment: DocComment {
                kind,
                text: self.source[start..self.pos].to_owned(),
                span: self.span_from(start),
                target: DocCommentTarget::Unattached,
            },
        });
    }

    /// Block comments end at the first closing marker and do not nest.
    fn skip_block_comment(&mut self) -> Result<(), Diagnostic> {
        let start = self.pos;
        self.pos += 2;
        loop {
            match self.peek_char() {
                None => {
                    return Err(Diagnostic::at(
                        self.span_from(start),
                        "unterminated block comment",
                    ));
                }
                Some('*') if self.source[self.pos + 1..].starts_with('/') => {
                    self.pos += 2;
                    return Ok(());
                }
                Some('\n') => {
                    self.pos += 1;
                    self.newline_before = true;
                }
                Some(c) => self.pos += c.len_utf8(),
            }
        }
    }
}
