use super::*;
use scoop_ast::StringPart;

impl Parser {
    pub(super) fn parse_interpolated_string(&mut self) -> Result<Expr, Diagnostic> {
        let open = self.bump();
        let mut parts = Vec::new();
        loop {
            let token = self.peek().clone();
            match token.kind {
                TokenKind::FStringText(value) => {
                    self.bump();
                    parts.push(StringPart::Text {
                        value,
                        span: token.span,
                    });
                }
                TokenKind::InterpolationStart => {
                    self.bump();
                    let value = Box::new(self.parse_expr()?);
                    let close = self.expect("`}` after interpolation expression", |kind| {
                        matches!(kind, TokenKind::InterpolationEnd)
                    })?;
                    parts.push(StringPart::Expression {
                        value,
                        span: Span::new(token.span.start, close.span.end),
                    });
                }
                TokenKind::FStringEnd => {
                    self.bump();
                    return Ok(Expr::InterpolatedString {
                        parts,
                        span: Span::new(open.span.start, token.span.end),
                    });
                }
                _ => return self.unexpected("f-string text, `${...}`, or closing quote"),
            }
        }
    }
}
