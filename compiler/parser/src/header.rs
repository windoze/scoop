use scoop_ast::{
    Diagnostic, ImportAliasSyntax, ImportExposureSyntax, ImportSyntax, PackageSyntax,
    QualifiedNameSyntax, QualifiedNameTailSyntax, Span,
};

use crate::{lexer::TokenKind, parser::Parser};

enum ImportSelector {
    Exact(QualifiedNameSyntax),
    Star {
        namespace: QualifiedNameSyntax,
        terminal_dot_span: Span,
        star_span: Span,
    },
}

impl Parser {
    pub(crate) fn parse_package_header(&mut self) -> Result<PackageSyntax, Diagnostic> {
        let keyword = self.expect("`package`", |kind| matches!(kind, TokenKind::Package))?;
        let path = self.parse_qualified_name("package name")?;
        let span = Span::new(keyword.span.start, path.span.end);
        Ok(PackageSyntax::QualifiedPackage {
            package_keyword_span: keyword.span,
            path,
            span,
        })
    }

    pub(crate) fn parse_import_header(&mut self) -> Result<ImportSyntax, Diagnostic> {
        let (start, exposure) = if self.at_public_import() {
            let public = self.bump();
            (
                public.span.start,
                ImportExposureSyntax::PublicReexport {
                    public_keyword_span: public.span,
                },
            )
        } else {
            (self.peek().span.start, ImportExposureSyntax::Local)
        };
        let keyword = self.expect("`import`", |kind| matches!(kind, TokenKind::Import))?;

        match self.parse_import_selector()? {
            ImportSelector::Exact(selector) => {
                let alias = if matches!(self.peek().kind, TokenKind::As) {
                    let as_keyword = self.bump();
                    let name = self.expect_ident("import alias")?;
                    Some(ImportAliasSyntax {
                        as_keyword_span: as_keyword.span,
                        span: Span::new(as_keyword.span.start, name.span.end),
                        name,
                    })
                } else {
                    None
                };
                let end = alias
                    .as_ref()
                    .map_or(selector.span.end, |alias| alias.span.end);
                Ok(ImportSyntax::Exact {
                    exposure,
                    selector,
                    alias,
                    import_keyword_span: keyword.span,
                    span: Span::new(start, end),
                })
            }
            ImportSelector::Star {
                namespace,
                terminal_dot_span,
                star_span,
            } => {
                if matches!(self.peek().kind, TokenKind::As) {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "star imports cannot have an alias",
                    ));
                }
                if matches!(self.peek().kind, TokenKind::Dot) {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "a star import selector must end at `*`",
                    ));
                }
                Ok(ImportSyntax::Star {
                    exposure,
                    namespace,
                    import_keyword_span: keyword.span,
                    terminal_dot_span,
                    star_span,
                    span: Span::new(start, star_span.end),
                })
            }
        }
    }

    fn parse_qualified_name(
        &mut self,
        first_expected: &str,
    ) -> Result<QualifiedNameSyntax, Diagnostic> {
        let first = self.expect_ident(first_expected)?;
        let start = first.span.start;
        let mut end = first.span.end;
        let mut rest = Vec::new();
        loop {
            if matches!(self.peek().kind, TokenKind::DotDot) {
                return Err(self.consecutive_dot_diagnostic());
            }
            if !matches!(self.peek().kind, TokenKind::Dot) {
                break;
            }
            let dot = self.bump();
            let identifier = self.expect_ident("identifier after `.`")?;
            end = identifier.span.end;
            rest.push(QualifiedNameTailSyntax {
                dot_span: dot.span,
                identifier,
            });
        }
        Ok(QualifiedNameSyntax {
            first,
            rest,
            span: Span::new(start, end),
        })
    }

    fn parse_import_selector(&mut self) -> Result<ImportSelector, Diagnostic> {
        let first = self.expect_ident("import selector")?;
        let start = first.span.start;
        let mut end = first.span.end;
        let mut rest = Vec::new();
        loop {
            if matches!(self.peek().kind, TokenKind::DotDot) {
                return Err(self.consecutive_dot_diagnostic());
            }
            if !matches!(self.peek().kind, TokenKind::Dot) {
                return Ok(ImportSelector::Exact(QualifiedNameSyntax {
                    first,
                    rest,
                    span: Span::new(start, end),
                }));
            }
            let dot = self.bump();
            if matches!(self.peek().kind, TokenKind::Star) {
                let star = self.bump();
                return Ok(ImportSelector::Star {
                    namespace: QualifiedNameSyntax {
                        first,
                        rest,
                        span: Span::new(start, end),
                    },
                    terminal_dot_span: dot.span,
                    star_span: star.span,
                });
            }
            let identifier = self.expect_ident("identifier after `.`")?;
            end = identifier.span.end;
            rest.push(QualifiedNameTailSyntax {
                dot_span: dot.span,
                identifier,
            });
        }
    }

    fn consecutive_dot_diagnostic(&self) -> Diagnostic {
        let token = self.peek();
        Diagnostic::at(
            Span::new(token.span.start + 1, token.span.end),
            "expected identifier after `.`, found `.`",
        )
    }
}
