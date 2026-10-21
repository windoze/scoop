use scoop_ast::{Diagnostic, DiagnosticNote, PackageSyntax, SourceFile};

use super::{HeaderState, Parser, TopLevelItemKind};

pub(crate) fn parse_file(source: &str) -> Result<SourceFile, Vec<Diagnostic>> {
    let (mut parser, span) = Parser::from_source(source)?;
    let mut package = PackageSyntax::RootPackage;
    let mut imports = Vec::new();
    let mut declarations = Vec::new();
    let mut header_state = HeaderState::Start;
    while !parser.at_eof() {
        let start = parser.pos;
        match parser.top_level_item_kind() {
            TopLevelItemKind::Package => {
                let keyword_span = parser.peek().span;
                if let PackageSyntax::QualifiedPackage {
                    package_keyword_span: first_keyword_span,
                    ..
                } = &package
                {
                    parser.diagnostics.push(
                        Diagnostic::at(
                            keyword_span,
                            "a source file may contain only one `package` header",
                        )
                        .with_note(DiagnosticNote::at(
                            0,
                            *first_keyword_span,
                            "first `package` header is here",
                        )),
                    );
                } else if header_state >= HeaderState::AfterImport {
                    parser.diagnostics.push(Diagnostic::at(
                        keyword_span,
                        "a `package` header must appear before imports and declarations",
                    ));
                }
                let accept = matches!(package, PackageSyntax::RootPackage)
                    && header_state < HeaderState::AfterImport;
                header_state.advance_to(HeaderState::AfterPackage);
                match parser.parse_package_header() {
                    Ok(parsed) if accept => {
                        package = parsed;
                    }
                    Ok(_) => {}
                    Err(diagnostic) => {
                        parser.diagnostics.push(diagnostic);
                        parser.synchronize_header_item(start);
                    }
                }
            }
            TopLevelItemKind::Import => {
                let header_span = parser.import_prefix_span();
                let accept = header_state < HeaderState::InDeclarations;
                if !accept {
                    parser.diagnostics.push(Diagnostic::at(
                        header_span,
                        "`import` headers must appear before declarations",
                    ));
                }
                header_state.advance_to(HeaderState::AfterImport);
                match parser.parse_import_header() {
                    Ok(import) if accept => imports.push(import),
                    Ok(_) => {}
                    Err(diagnostic) => {
                        parser.diagnostics.push(diagnostic);
                        parser.synchronize_header_item(start);
                    }
                }
            }
            TopLevelItemKind::UnsupportedImportModifier(modifier) => {
                let modifier_token = parser.bump();
                if header_state >= HeaderState::InDeclarations {
                    parser.diagnostics.push(Diagnostic::at(
                        modifier_token.span,
                        "`import` headers must appear before declarations",
                    ));
                }
                parser.diagnostics.push(Diagnostic::at(
                    modifier_token.span,
                    format!(
                        "`{modifier} import` is not supported; imports may be ordinary or `public`"
                    ),
                ));
                header_state.advance_to(HeaderState::AfterImport);
                match parser.parse_import_header() {
                    Ok(_) => {}
                    Err(diagnostic) => {
                        parser.diagnostics.push(diagnostic);
                        parser.synchronize_header_item(start);
                    }
                }
            }
            TopLevelItemKind::Declaration => {
                header_state.advance_to(HeaderState::InDeclarations);
                match parser.parse_decl() {
                    Ok(decl) => {
                        parser.attach_documentation(start, decl.span());
                        declarations.push(decl);
                    }
                    Err(diagnostic) => {
                        parser.diagnostics.push(diagnostic);
                        parser.synchronize_top_level(start);
                    }
                }
            }
        }
    }
    let file = SourceFile {
        package,
        imports,
        declarations,
        documentation: std::mem::take(&mut parser.documentation)
            .into_iter()
            .map(|doc| doc.comment)
            .collect(),
        span,
    };
    parser.finish(Ok(file))
}
