use crate::{Ident, Span};

/// The package declared by one source file.
///
/// The root package is represented explicitly rather than by a missing or
/// empty qualified name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PackageSyntax {
    #[default]
    RootPackage,
    QualifiedPackage {
        package_keyword_span: Span,
        path: QualifiedNameSyntax,
        span: Span,
    },
}

/// A non-empty dotted identifier path together with every separator span.
///
/// Keeping the first segment outside `rest` makes an empty path impossible;
/// pairing every later identifier with its leading dot prevents the two span
/// sequences from getting out of sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedNameSyntax {
    pub first: Ident,
    pub rest: Vec<QualifiedNameTailSyntax>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedNameTailSyntax {
    pub dot_span: Span,
    pub identifier: Ident,
}

impl QualifiedNameSyntax {
    pub fn segments(&self) -> impl Iterator<Item = &Ident> {
        std::iter::once(&self.first).chain(self.rest.iter().map(|tail| &tail.identifier))
    }
}

/// One syntactically complete import header.
///
/// Exact and star imports are separate variants, so a successful star import
/// cannot carry an alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSyntax {
    Exact {
        exposure: ImportExposureSyntax,
        selector: QualifiedNameSyntax,
        alias: Option<ImportAliasSyntax>,
        import_keyword_span: Span,
        span: Span,
    },
    Star {
        exposure: ImportExposureSyntax,
        namespace: QualifiedNameSyntax,
        import_keyword_span: Span,
        terminal_dot_span: Span,
        star_span: Span,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportExposureSyntax {
    Local,
    PublicReexport { public_keyword_span: Span },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportAliasSyntax {
    pub as_keyword_span: Span,
    pub name: Ident,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(text: &str, start: u32, end: u32) -> Ident {
        Ident {
            text: text.to_string(),
            span: Span::new(start, end),
        }
    }

    #[test]
    fn qualified_name_is_non_empty_and_retains_dot_spans() {
        let name = QualifiedNameSyntax {
            first: ident("dev", 0, 3),
            rest: vec![
                QualifiedNameTailSyntax {
                    dot_span: Span::new(3, 4),
                    identifier: ident("example", 4, 11),
                },
                QualifiedNameTailSyntax {
                    dot_span: Span::new(11, 12),
                    identifier: ident("api", 12, 15),
                },
            ],
            span: Span::new(0, 15),
        };

        assert_eq!(
            name.segments()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>(),
            ["dev", "example", "api"]
        );
        assert_eq!(name.rest[0].dot_span, Span::new(3, 4));
        assert_eq!(name.rest[1].dot_span, Span::new(11, 12));
    }

    #[test]
    fn qualified_package_retains_keyword_path_and_full_spans() {
        let package = PackageSyntax::QualifiedPackage {
            package_keyword_span: Span::new(0, 7),
            path: QualifiedNameSyntax {
                first: ident("dev", 8, 11),
                rest: Vec::new(),
                span: Span::new(8, 11),
            },
            span: Span::new(0, 11),
        };

        let PackageSyntax::QualifiedPackage {
            package_keyword_span,
            path,
            span,
        } = package
        else {
            unreachable!()
        };
        assert_eq!(package_keyword_span, Span::new(0, 7));
        assert_eq!(path.first.span, Span::new(8, 11));
        assert_eq!(path.span, Span::new(8, 11));
        assert_eq!(span, Span::new(0, 11));
    }

    #[test]
    fn exact_import_retains_public_alias_and_full_spans() {
        let import = ImportSyntax::Exact {
            exposure: ImportExposureSyntax::PublicReexport {
                public_keyword_span: Span::new(0, 6),
            },
            selector: QualifiedNameSyntax {
                first: ident("dev", 14, 17),
                rest: vec![QualifiedNameTailSyntax {
                    dot_span: Span::new(17, 18),
                    identifier: ident("User", 18, 22),
                }],
                span: Span::new(14, 22),
            },
            alias: Some(ImportAliasSyntax {
                as_keyword_span: Span::new(23, 25),
                name: ident("Person", 26, 32),
                span: Span::new(23, 32),
            }),
            import_keyword_span: Span::new(7, 13),
            span: Span::new(0, 32),
        };

        let ImportSyntax::Exact {
            exposure,
            selector,
            alias,
            import_keyword_span,
            span,
        } = import
        else {
            unreachable!()
        };
        assert_eq!(
            exposure,
            ImportExposureSyntax::PublicReexport {
                public_keyword_span: Span::new(0, 6)
            }
        );
        assert_eq!(selector.rest[0].dot_span, Span::new(17, 18));
        assert_eq!(selector.span, Span::new(14, 22));
        let alias = alias.expect("the exact import carries an alias");
        assert_eq!(alias.as_keyword_span, Span::new(23, 25));
        assert_eq!(alias.name.span, Span::new(26, 32));
        assert_eq!(alias.span, Span::new(23, 32));
        assert_eq!(import_keyword_span, Span::new(7, 13));
        assert_eq!(span, Span::new(0, 32));
    }

    #[test]
    fn star_import_owns_its_exposure_keyword_terminal_and_full_spans() {
        let import = ImportSyntax::Star {
            exposure: ImportExposureSyntax::PublicReexport {
                public_keyword_span: Span::new(0, 6),
            },
            namespace: QualifiedNameSyntax {
                first: ident("dev", 14, 17),
                rest: Vec::new(),
                span: Span::new(14, 17),
            },
            import_keyword_span: Span::new(7, 13),
            terminal_dot_span: Span::new(17, 18),
            star_span: Span::new(18, 19),
            span: Span::new(0, 19),
        };

        let ImportSyntax::Star {
            exposure,
            import_keyword_span,
            terminal_dot_span,
            star_span,
            span,
            ..
        } = import
        else {
            unreachable!()
        };
        assert_eq!(
            exposure,
            ImportExposureSyntax::PublicReexport {
                public_keyword_span: Span::new(0, 6)
            }
        );
        assert_eq!(import_keyword_span, Span::new(7, 13));
        assert_eq!(terminal_dot_span, Span::new(17, 18));
        assert_eq!(star_span, Span::new(18, 19));
        assert_eq!(span, Span::new(0, 19));
    }
}
