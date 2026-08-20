use super::super::*;

pub(super) fn dump_headers(file: &SourceFile, out: &mut String) {
    match &file.package {
        PackageSyntax::RootPackage => out.push_str("  RootPackage\n"),
        PackageSyntax::QualifiedPackage { path, .. } => {
            out.push_str("  QualifiedPackage ");
            dump_qualified_name(path, out);
            out.push('\n');
        }
    }

    for import in &file.imports {
        match import {
            ImportSyntax::Exact {
                exposure,
                selector,
                alias,
                ..
            } => {
                out.push_str("  ImportExact ");
                out.push_str(dump_exposure(*exposure));
                out.push(' ');
                dump_qualified_name(selector, out);
                if let Some(alias) = alias {
                    out.push_str(" as ");
                    out.push_str(&alias.name.text);
                }
                out.push('\n');
            }
            ImportSyntax::Star {
                exposure,
                namespace,
                ..
            } => {
                out.push_str("  ImportStar ");
                out.push_str(dump_exposure(*exposure));
                out.push(' ');
                dump_qualified_name(namespace, out);
                out.push_str(".*\n");
            }
        }
    }
}

fn dump_exposure(exposure: ImportExposureSyntax) -> &'static str {
    match exposure {
        ImportExposureSyntax::Local => "Local",
        ImportExposureSyntax::PublicReexport { .. } => "PublicReexport",
    }
}

fn dump_qualified_name(name: &QualifiedNameSyntax, out: &mut String) {
    out.push_str(&name.first.text);
    for tail in &name.rest {
        out.push('.');
        out.push_str(&tail.identifier.text);
    }
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

    fn qualified(first: Ident, rest: Vec<(Span, Ident)>, span: Span) -> QualifiedNameSyntax {
        QualifiedNameSyntax {
            first,
            rest: rest
                .into_iter()
                .map(|(dot_span, identifier)| QualifiedNameTailSyntax {
                    dot_span,
                    identifier,
                })
                .collect(),
            span,
        }
    }

    #[test]
    fn dump_makes_the_root_package_explicit() {
        let file = SourceFile {
            package: PackageSyntax::RootPackage,
            imports: Vec::new(),
            declarations: Vec::new(),
            span: Span::new(0, 0),
        };

        assert_eq!(crate::dump(&file), "SourceFile\n  RootPackage\n");
    }

    #[test]
    fn dump_preserves_header_variants_exposure_and_alias() {
        let file = SourceFile {
            package: PackageSyntax::QualifiedPackage {
                package_keyword_span: Span::new(0, 7),
                path: qualified(
                    ident("dev", 8, 11),
                    vec![(Span::new(11, 12), ident("example", 12, 19))],
                    Span::new(8, 19),
                ),
                span: Span::new(0, 19),
            },
            imports: vec![
                ImportSyntax::Exact {
                    exposure: ImportExposureSyntax::Local,
                    selector: qualified(
                        ident("dev", 27, 30),
                        vec![(Span::new(30, 31), ident("User", 31, 35))],
                        Span::new(27, 35),
                    ),
                    alias: Some(ImportAliasSyntax {
                        as_keyword_span: Span::new(36, 38),
                        name: ident("Person", 39, 45),
                        span: Span::new(36, 45),
                    }),
                    import_keyword_span: Span::new(20, 26),
                    span: Span::new(20, 45),
                },
                ImportSyntax::Star {
                    exposure: ImportExposureSyntax::PublicReexport {
                        public_keyword_span: Span::new(46, 52),
                    },
                    namespace: qualified(
                        ident("dev", 60, 63),
                        vec![(Span::new(63, 64), ident("api", 64, 67))],
                        Span::new(60, 67),
                    ),
                    import_keyword_span: Span::new(53, 59),
                    terminal_dot_span: Span::new(67, 68),
                    star_span: Span::new(68, 69),
                    span: Span::new(46, 69),
                },
            ],
            declarations: Vec::new(),
            span: Span::new(0, 69),
        };

        assert_eq!(
            crate::dump(&file),
            concat!(
                "SourceFile\n",
                "  QualifiedPackage dev.example\n",
                "  ImportExact Local dev.User as Person\n",
                "  ImportStar PublicReexport dev.api.*\n",
            )
        );
    }
}
