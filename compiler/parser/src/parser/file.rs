use scoop_ast::{
    DeclarationFragment, Diagnostic, DiagnosticNote, DiagnosticSeverity, PackageSyntax,
    RecoveredFileHeader, SourceFile,
};
use scoop_source::SourceLengthError;
use scoop_syntax::{DeclarationData, Parse, SyntaxKind};

use super::{HeaderState, Parser, TopLevelItemKind};

struct FileRecovery {
    package_complete: bool,
    imports_complete: bool,
    declarations_complete: Vec<bool>,
}

pub(crate) fn parse_file(source: &str) -> Result<crate::ParsedFile, SourceLengthError> {
    let (mut parser, span) = Parser::from_source(source)?;
    let mut package = PackageSyntax::RootPackage;
    let mut imports = Vec::new();
    let mut declarations = Vec::new();
    let mut header_state = HeaderState::Start;
    let mut recovery = FileRecovery {
        package_complete: true,
        imports_complete: true,
        declarations_complete: Vec::new(),
    };
    while !parser.at_eof() {
        let start = parser.pos;
        let checkpoint = parser.checkpoint();
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
                recovery.package_complete &= parser.fragment_is_complete(checkpoint);
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
                recovery.imports_complete &= parser.fragment_is_complete(checkpoint);
            }
            TopLevelItemKind::UnsupportedImportModifier(modifier) => {
                recovery.imports_complete = false;
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
                        recovery
                            .declarations_complete
                            .push(parser.fragment_is_complete(checkpoint));
                        parser.record_top_level_declaration();
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
    Ok(finish(parser, source, file, recovery))
}

fn finish(
    parser: Parser,
    source: &str,
    file: SourceFile,
    recovery: FileRecovery,
) -> crate::ParsedFile {
    let (syntax, diagnostics) = parser.finish_syntax(source, SyntaxKind::SourceFile);
    let mut projections = syntax
        .function_headers
        .into_iter()
        .map(|header| DeclarationData {
            declaration: header.declaration,
            value: DeclarationFragment::FunctionHeader(Box::new(header.value)),
        })
        .collect::<Vec<_>>();
    let (ast, recovered) = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        let SourceFile {
            package,
            imports,
            declarations,
            documentation,
            ..
        } = file;
        assert_eq!(
            declarations.len(),
            syntax.top_level_declarations.len(),
            "each parsed top-level declaration has its grammar marker"
        );
        for ((value, complete), declaration) in declarations
            .into_iter()
            .zip(recovery.declarations_complete)
            .zip(syntax.top_level_declarations)
        {
            if complete {
                projections.push(DeclarationData {
                    declaration,
                    value: DeclarationFragment::Complete(Box::new(value)),
                });
            }
        }
        let mut complete = vec![false; syntax.declarations.len()];
        let mut known_signature = vec![false; syntax.declarations.len()];
        for projection in &projections {
            match &projection.value {
                DeclarationFragment::Complete(_) => complete[projection.declaration.0] = true,
                DeclarationFragment::FunctionHeader(_) => {
                    known_signature[projection.declaration.0] = true
                }
                DeclarationFragment::Unavailable(_)
                | DeclarationFragment::FunctionBody(_)
                | DeclarationFragment::Class(_) => {
                    unreachable!("unavailable headers are added below")
                }
            }
        }
        for (index, declaration) in syntax.declarations.iter().enumerate() {
            if let Some(parent) = declaration.parent {
                complete[index] |= complete[parent.0];
            }
        }
        for class in syntax.classes {
            if !complete[class.declaration.0] {
                known_signature[class.declaration.0] = true;
                projections.push(DeclarationData {
                    declaration: class.declaration,
                    value: DeclarationFragment::Class(Box::new(class.value)),
                });
            }
        }
        for header in syntax.declaration_headers {
            if !complete[header.declaration.0] && !known_signature[header.declaration.0] {
                projections.push(DeclarationData {
                    declaration: header.declaration,
                    value: DeclarationFragment::Unavailable(header.value),
                });
            }
        }
        for body in syntax.bodies {
            if !complete[body.declaration.0] && known_signature[body.declaration.0] {
                projections.push(DeclarationData {
                    declaration: body.declaration,
                    value: DeclarationFragment::FunctionBody(Box::new(body.value)),
                });
            }
        }
        (
            None,
            Some(RecoveredFileHeader {
                package: recovery.package_complete.then_some(package),
                imports,
                imports_complete: recovery.imports_complete,
                documentation,
            }),
        )
    } else {
        (Some(file), None)
    };
    projections.sort_by_key(|projection| projection.declaration.0);
    Parse::new(
        syntax.green,
        diagnostics,
        syntax.missing,
        syntax.declarations,
        ast,
    )
    .with_declaration_data(crate::FileProjections {
        source: recovered,
        declarations: projections,
    })
}
