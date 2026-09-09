use scoop_ast::{
    Decl, DeclaredVisibility, ImportExposureSyntax, ImportSyntax, NonEmptyVec, PackageSyntax, Span,
    VisibilitySyntax,
};
use scoop_identity::{ConeCoordinate, NormalizedSourcePath, SourceIdentity};

use crate::{IdentifiedSourceInput, ParserDiagnosticContext, parse, parse_all};

fn source_identity(path: &str) -> SourceIdentity {
    SourceIdentity::new(
        ConeCoordinate::new("test", "parser-headers", "0.0.0")
            .unwrap()
            .identity()
            .unwrap(),
        NormalizedSourcePath::new(path).unwrap(),
    )
    .unwrap()
}

fn ok(source: &str) -> scoop_ast::SourceFile {
    parse(source).unwrap_or_else(|diagnostics| panic!("should parse: {diagnostics:?}"))
}

fn only_error(source: &str) -> (Span, String) {
    let diagnostics = parse(source).expect_err("source should fail");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let diagnostic = &diagnostics[0];
    (
        diagnostic.span.expect("parser error has a span"),
        diagnostic.message.clone(),
    )
}

fn occurrence_span(source: &str, needle: &str, occurrence: usize) -> Span {
    let (start, _) = source
        .match_indices(needle)
        .nth(occurrence)
        .unwrap_or_else(|| panic!("missing occurrence {occurrence} of {needle:?}"));
    Span::new(start as u32, (start + needle.len()) as u32)
}

fn path_text(path: &scoop_ast::QualifiedNameSyntax) -> Vec<&str> {
    path.segments()
        .map(|segment| segment.text.as_str())
        .collect()
}

#[test]
fn omitted_package_is_an_explicit_root_package() {
    let file = ok("fun main() {}");
    assert_eq!(file.package, PackageSyntax::RootPackage);
    assert!(file.imports.is_empty());
}

#[test]
fn package_and_import_are_dedicated_keywords() {
    let (_, package_message) = only_error("fun package() {}");
    assert_eq!(package_message, "expected function name, found `package`");

    let (_, import_message) = only_error("fun import() {}");
    assert_eq!(import_message, "expected function name, found `import`");
}

#[test]
fn parses_every_header_form_and_keeps_public_declarations_distinct() {
    let source = "package app.main\n\
                  import model.User\n\
                  import render.draw as drawUser\n\
                  import model.states.*\n\
                  public import api.Result\n\
                  public import api.render as publicRender\n\
                  public import api.errors.*\n\
                  public fun main() {}\n";
    let file = ok(source);

    assert_eq!(file.imports.len(), 6);
    assert_eq!(
        scoop_ast::dump(&file),
        concat!(
            "SourceFile\n",
            "  QualifiedPackage app.main\n",
            "  ImportExact Local model.User\n",
            "  ImportExact Local render.draw as drawUser\n",
            "  ImportStar Local model.states.*\n",
            "  ImportExact PublicReexport api.Result\n",
            "  ImportExact PublicReexport api.render as publicRender\n",
            "  ImportStar PublicReexport api.errors.*\n",
            "  public fun main()\n",
        )
    );
    let Decl::Function(main) = &file.declarations[0] else {
        panic!("expected main function")
    };
    assert!(matches!(
        main.visibility,
        VisibilitySyntax::Explicit {
            visibility: DeclaredVisibility::Public,
            ..
        }
    ));
}

#[test]
fn package_and_import_nodes_retain_every_token_span() {
    let package_source = "package dev.example";
    let package_file = ok(package_source);
    let PackageSyntax::QualifiedPackage {
        package_keyword_span,
        path,
        span,
    } = &package_file.package
    else {
        panic!("expected qualified package")
    };
    assert_eq!(*package_keyword_span, Span::new(0, 7));
    assert_eq!(path_text(path), ["dev", "example"]);
    assert_eq!(path.first.span, Span::new(8, 11));
    assert_eq!(path.rest[0].dot_span, Span::new(11, 12));
    assert_eq!(path.rest[0].identifier.span, Span::new(12, 19));
    assert_eq!(path.span, Span::new(8, 19));
    assert_eq!(*span, Span::new(0, 19));

    let exact_source = "public import dev.User as Person";
    let exact_file = ok(exact_source);
    let ImportSyntax::Exact {
        exposure,
        selector,
        alias,
        import_keyword_span,
        span,
    } = &exact_file.imports[0]
    else {
        panic!("expected exact import")
    };
    assert_eq!(
        *exposure,
        ImportExposureSyntax::PublicReexport {
            public_keyword_span: Span::new(0, 6)
        }
    );
    assert_eq!(*import_keyword_span, Span::new(7, 13));
    assert_eq!(path_text(selector), ["dev", "User"]);
    assert_eq!(selector.first.span, Span::new(14, 17));
    assert_eq!(selector.rest[0].dot_span, Span::new(17, 18));
    assert_eq!(selector.rest[0].identifier.span, Span::new(18, 22));
    assert_eq!(selector.span, Span::new(14, 22));
    let alias = alias.as_ref().expect("exact import has an alias");
    assert_eq!(alias.as_keyword_span, Span::new(23, 25));
    assert_eq!(alias.name.span, Span::new(26, 32));
    assert_eq!(alias.span, Span::new(23, 32));
    assert_eq!(*span, Span::new(0, 32));

    let star_source = "import dev.api.*";
    let star_file = ok(star_source);
    let ImportSyntax::Star {
        exposure,
        namespace,
        import_keyword_span,
        terminal_dot_span,
        star_span,
        span,
    } = &star_file.imports[0]
    else {
        panic!("expected star import")
    };
    assert_eq!(*exposure, ImportExposureSyntax::Local);
    assert_eq!(*import_keyword_span, Span::new(0, 6));
    assert_eq!(path_text(namespace), ["dev", "api"]);
    assert_eq!(namespace.rest[0].dot_span, Span::new(10, 11));
    assert_eq!(*terminal_dot_span, Span::new(14, 15));
    assert_eq!(*star_span, Span::new(15, 16));
    assert_eq!(*span, Span::new(0, 16));
}

#[test]
fn trivia_does_not_change_header_dump_semantics() {
    let compact = ok("package dev.example\nimport dev.User as Person\nfun main() {}");
    let with_trivia = ok("package /* one */ dev /* two */ . /* three */ example\n\
         import /* four */ dev . User /* five */ as /* six */ Person\n\
         fun main() {}");
    assert_eq!(scoop_ast::dump(&compact), scoop_ast::dump(&with_trivia));
}

#[test]
fn malformed_header_paths_have_focused_spans_and_messages() {
    let cases = [
        (
            "package",
            Span::new(7, 7),
            "expected package name, found end of file",
        ),
        (
            "package .a",
            Span::new(8, 9),
            "expected package name, found `.`",
        ),
        (
            "package a.",
            Span::new(10, 10),
            "expected identifier after `.`, found end of file",
        ),
        (
            "import",
            Span::new(6, 6),
            "expected import selector, found end of file",
        ),
        (
            "import *",
            Span::new(7, 8),
            "expected import selector, found `*`",
        ),
        (
            "import .a",
            Span::new(7, 8),
            "expected import selector, found `.`",
        ),
        (
            "import a.",
            Span::new(9, 9),
            "expected identifier after `.`, found end of file",
        ),
        (
            "import a..b",
            Span::new(9, 10),
            "expected identifier after `.`, found `.`",
        ),
        (
            "import a.*.b",
            Span::new(10, 11),
            "a star import selector must end at `*`",
        ),
        (
            "import a.* as x",
            Span::new(11, 13),
            "star imports cannot have an alias",
        ),
        (
            "import a.B as",
            Span::new(13, 13),
            "expected import alias, found end of file",
        ),
        (
            "public import",
            Span::new(13, 13),
            "expected import selector, found end of file",
        ),
    ];

    for (source, expected_span, expected_message) in cases {
        let (actual_span, actual_message) = only_error(source);
        assert_eq!(actual_span, expected_span, "source: {source}");
        assert_eq!(actual_message, expected_message, "source: {source}");
    }
}

#[test]
fn import_visibility_modifiers_have_targeted_diagnostics() {
    for modifier in ["internal", "private"] {
        let source = format!("{modifier} import dev.User");
        let (span, message) = only_error(&source);
        assert_eq!(span, Span::new(0, modifier.len() as u32));
        assert_eq!(
            message,
            format!("`{modifier} import` is not supported; imports may be ordinary or `public`")
        );
    }
}

#[test]
fn duplicate_and_misplaced_headers_have_stable_priority() {
    let duplicate = parse("package one\npackage two").expect_err("duplicate package must fail");
    assert_eq!(duplicate.len(), 1, "{duplicate:?}");
    assert_eq!(
        duplicate[0].message,
        "a source file may contain only one `package` header"
    );
    assert_eq!(
        duplicate[0].span,
        Some(occurrence_span("package one\npackage two", "package", 1))
    );
    assert_eq!(
        duplicate[0].notes,
        [scoop_ast::DiagnosticNote::at(
            0,
            Span::new(0, 7),
            "first `package` header is here",
        )]
    );

    let malformed_then_valid =
        parse("package .bad\npackage good").expect_err("the malformed first header must fail");
    assert_eq!(malformed_then_valid.len(), 1, "{malformed_then_valid:?}");
    assert_eq!(
        malformed_then_valid[0].message,
        "expected package name, found `.`"
    );
    assert!(malformed_then_valid[0].notes.is_empty());

    let after_import =
        parse("import one.Value\npackage later").expect_err("late package must fail");
    assert_eq!(after_import.len(), 1, "{after_import:?}");
    assert_eq!(
        after_import[0].message,
        "a `package` header must appear before imports and declarations"
    );
    assert!(after_import[0].notes.is_empty());

    let source = "fun main() {}\nimport one.Value\npublic import two.Value";
    let after_declaration = parse(source).expect_err("late imports must fail");
    assert_eq!(after_declaration.len(), 2, "{after_declaration:?}");
    assert_eq!(
        after_declaration[0].span,
        Some(occurrence_span(source, "import", 0))
    );
    assert_eq!(
        after_declaration[1].span,
        Some(Span::new(
            occurrence_span(source, "public", 0).start,
            occurrence_span(source, "import", 1).end,
        ))
    );
    assert!(
        after_declaration
            .iter()
            .all(|diagnostic| diagnostic.message
                == "`import` headers must appear before declarations")
    );
}

#[test]
fn malformed_headers_recover_at_balanced_top_level_boundaries() {
    let source = "import a.(\n\
                  fun swallowed(: Int)\n\
                  )\n\
                  import .bad\n\
                  fun retained(: Int) {}";
    let diagnostics = parse(source).expect_err("three independent roots must be reported");
    let messages: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "expected identifier after `.`, found `(`",
            "expected import selector, found `.`",
            "expected parameter name, found `:`",
        ]
    );
    assert!(diagnostics.windows(2).all(|pair| {
        pair[0].span.expect("spanned").start < pair[1].span.expect("spanned").start
    }));
}

#[test]
fn header_recovery_does_not_split_on_same_line_contextual_modifiers() {
    let source = "import . public.value\nfun retained(: Int) {}";
    let diagnostics = parse(source).expect_err("both independent roots must be reported");
    let messages: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "expected import selector, found `.`",
            "expected parameter name, found `:`",
        ]
    );
}

#[test]
fn top_level_recovery_recognizes_val_and_var_starters() {
    let source = "fun broken(: Int) {}\nval = 1\nvar = 2";
    let diagnostics = parse(source).expect_err("all three declarations are malformed");
    assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
    assert!(diagnostics.windows(2).all(|pair| {
        pair[0].span.expect("spanned").start < pair[1].span.expect("spanned").start
    }));
}

#[test]
fn lexical_and_syntax_diagnostics_are_merged_in_source_order() {
    let source = "§\nfun first(: Int) {}\n$\nfun second(: Int) {}";
    let diagnostics = parse(source).expect_err("lexical and syntax errors must all be returned");
    let messages: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "unexpected character `§`",
            "expected parameter name, found `:`",
            "unexpected character `$`",
            "expected parameter name, found `:`",
        ]
    );
    assert!(diagnostics.windows(2).all(|pair| {
        pair[0].span.expect("spanned").start < pair[1].span.expect("spanned").start
    }));
}

#[test]
fn parse_all_returns_only_a_validated_non_empty_semantic_source_set() {
    let first_identity = source_identity("src/one.scoop");
    let second_identity = source_identity("src/two.scoop");
    let parsed = parse_all(NonEmptyVec::new(
        IdentifiedSourceInput::new(&first_identity, "package one\nfun helper() {}"),
        vec![IdentifiedSourceInput::new(
            &second_identity,
            "package two\nfun main() {}",
        )],
    ))
    .expect("both explicitly selected sources parse");

    assert_eq!(parsed.sources().len(), 2);
    assert_eq!(parsed.sources().as_slice()[0].identity(), &first_identity);
    assert_eq!(parsed.sources().as_slice()[1].identity(), &second_identity);
    let PackageSyntax::QualifiedPackage { path, .. } =
        &parsed.sources().as_slice()[1].ast().package
    else {
        panic!("second source has a qualified package")
    };
    assert_eq!(path_text(path), ["two"]);
}

#[test]
fn parse_all_rejects_duplicate_identities_but_parses_every_source() {
    let first_identity = source_identity("src/first.scoop");
    let other_identity = source_identity("src/other.scoop");
    let errors = parse_all(NonEmptyVec::new(
        IdentifiedSourceInput::new(&first_identity, "package .bad"),
        vec![
            IdentifiedSourceInput::new(&first_identity, "fun duplicate(: Int) {}"),
            IdentifiedSourceInput::new(&other_identity, "$\nfun other(: Int) {}"),
        ],
    ))
    .expect_err("no partial AST set may escape input or parser errors");

    let messages: Vec<_> = errors
        .iter()
        .map(|error| error.diagnostic().message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "expected package name, found `.`",
            &format!(
                "duplicate source identity {}/src/first.scoop (first used by source 0)",
                first_identity.cone()
            ),
            "expected parameter name, found `:`",
            "unexpected character `$`",
            "expected parameter name, found `:`",
        ]
    );
    assert_eq!(
        errors
            .iter()
            .map(|error| error.diagnostic().file)
            .collect::<Vec<_>>(),
        [0, 1, 1, 2, 2]
    );
    let context = ParserDiagnosticContext::new([
        (first_identity, "duplicate.scoop".to_string()),
        (other_identity, "other.scoop".to_string()),
    ]);
    assert_eq!(
        context.display_locator(errors[1].identity()),
        Some("duplicate.scoop")
    );
    assert_eq!(
        context.display_locator(errors[3].identity()),
        Some("other.scoop")
    );
}
