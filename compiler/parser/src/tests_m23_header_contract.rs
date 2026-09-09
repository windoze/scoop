use scoop_ast::{ImportSyntax, NonEmptyVec, Span};
use scoop_identity::{ConeCoordinate, NormalizedSourcePath, SourceIdentity};

use crate::{IdentifiedSourceInput, ParserDiagnosticContext, parse, parse_all};

fn source_identity(path: &str) -> SourceIdentity {
    SourceIdentity::new(
        ConeCoordinate::new("test", "parser-contract", "0.0.0")
            .unwrap()
            .identity()
            .unwrap(),
        NormalizedSourcePath::new(path).unwrap(),
    )
    .unwrap()
}

fn span_of(source: &str, token: &str) -> Span {
    let start = source.find(token).expect("token exists") as u32;
    Span::new(start, start + token.len() as u32)
}

#[test]
fn headers_preserve_every_existing_public_declaration_form() {
    let declarations = [
        "public fun main() {}",
        "public struct Point(val x: Int)",
        "public enum State { Ready }",
        "public class Widget {}",
        "public interface Renderable {}",
        "public object Registry {}",
        "public typealias Count = Int",
        "public val answer: Int = 42",
        "public var counter: Int = 0",
    ];
    for declaration in declarations {
        let source = format!(
            "package app\nimport library.Base\npublic /* exposure */\nimport library.Item\n{declaration}"
        );
        let file = parse(&source).expect("headers and public declaration parse");
        assert_eq!(file.imports.len(), 2);
        assert_eq!(file.declarations.len(), 1);
        let standalone = parse(declaration).expect("standalone declaration parses");
        let declaration_dump = scoop_ast::dump(&standalone);
        let declaration_dump = declaration_dump
            .strip_prefix("SourceFile\n  RootPackage\n")
            .expect("root dump prefix");
        assert_eq!(
            scoop_ast::dump(&file),
            format!(
                "SourceFile\n  QualifiedPackage app\n  ImportExact Local library.Base\n  ImportExact PublicReexport library.Item\n{declaration_dump}"
            ),
            "{declaration}"
        );
    }
}

#[test]
fn public_star_spans_include_trivia_and_use_utf8_byte_offsets() {
    let source = "/* 中文 */ public /* p */ import model /* m */ . /* d */ *";
    let file = parse(source).expect("trivia is accepted between header tokens");
    let ImportSyntax::Star {
        exposure,
        namespace,
        import_keyword_span,
        terminal_dot_span,
        star_span,
        span,
    } = &file.imports[0]
    else {
        panic!("expected star import")
    };
    assert_eq!(
        *exposure,
        scoop_ast::ImportExposureSyntax::PublicReexport {
            public_keyword_span: span_of(source, "public"),
        }
    );
    assert_eq!(*import_keyword_span, span_of(source, "import"));
    assert_eq!(namespace.span, span_of(source, "model"));
    assert_eq!(*terminal_dot_span, span_of(source, "."));
    assert_eq!(
        *star_span,
        Span::new(source.len() as u32 - 1, source.len() as u32)
    );
    assert_eq!(
        *span,
        Span::new(span_of(source, "public").start, source.len() as u32)
    );
    assert_eq!(file.span, Span::new(0, source.len() as u32));
}

#[test]
fn malformed_headers_and_declarations_keep_independent_diagnostics() {
    let source = "package .bad\nimport missing..Item\npublic import empty.* as Alias\nfun broken(: Int) {}\nimport later.Item\npackage later";
    let errors = parse(source).expect_err("recovery cannot publish an AST");
    let expected = [
        (
            span_of(source, ".bad").start,
            "expected package name, found `.`",
        ),
        (
            span_of(source, "..").start + 1,
            "expected identifier after `.`, found `.`",
        ),
        (
            span_of(source, "as Alias").start,
            "star imports cannot have an alias",
        ),
        (
            span_of(source, ": Int").start,
            "expected parameter name, found `:`",
        ),
        (
            span_of(source, "import later").start,
            "`import` headers must appear before declarations",
        ),
        (
            span_of(source, "package later").start,
            "a `package` header must appear before imports and declarations",
        ),
    ];
    assert_eq!(errors.len(), expected.len(), "{errors:?}");
    for (error, (start, message)) in errors.iter().zip(expected) {
        assert_eq!(error.message, message);
        assert_eq!(error.span.expect("spanned error").start, start);
    }
}

#[test]
fn duplicate_package_priority_survives_imports_and_declaration_recovery() {
    let source = "package first\nimport lib.Item\npackage second\nfun broken(: Int) {}\npackage third\npublic import lib.Other";
    let errors = parse(source).expect_err("duplicate and misplaced headers fail");
    let messages: Vec<_> = errors.iter().map(|error| error.message.as_str()).collect();
    assert_eq!(
        messages,
        [
            "a source file may contain only one `package` header",
            "expected parameter name, found `:`",
            "a source file may contain only one `package` header",
            "`import` headers must appear before declarations",
        ]
    );
    for index in [0, 2] {
        assert_eq!(
            errors[index].notes,
            [scoop_ast::DiagnosticNote::at(
                0,
                Span::new(0, 7),
                "first `package` header is here",
            )]
        );
    }
    assert!(errors[1].notes.is_empty());
    assert!(errors[3].notes.is_empty());
}

#[test]
fn duplicate_package_note_tracks_the_first_valid_header_and_multi_source_index() {
    let first = source_identity("src/first.scoop");
    let second = source_identity("src/second.scoop");
    let text = "/* 中文 */ package .broken\npackage valid\nimport lib.Item\npackage duplicate";
    let errors = parse_all(NonEmptyVec::new(
        IdentifiedSourceInput::new(&first, "package other"),
        vec![IdentifiedSourceInput::new(&second, text)],
    ))
    .expect_err("no AST set escapes malformed or duplicate headers");
    assert_eq!(errors.len(), 2);
    assert_eq!(
        errors[0].diagnostic().message,
        "expected package name, found `.`"
    );
    assert!(errors[0].diagnostic().notes.is_empty());
    let duplicate = errors[1].diagnostic();
    assert_eq!(duplicate.file, 1);
    let primary = span_of(text, "package duplicate").start;
    assert_eq!(duplicate.span, Some(Span::new(primary, primary + 7)));
    let first_valid = span_of(text, "package valid").start;
    assert_eq!(
        duplicate.notes,
        [scoop_ast::DiagnosticNote::at(
            1,
            Span::new(first_valid, first_valid + 7),
            "first `package` header is here",
        )]
    );
    assert_eq!(errors[1].identity(), &second);
    let context = ParserDiagnosticContext::new([
        (first, "first.scoop".to_string()),
        (second.clone(), "second.scoop".to_string()),
    ]);
    assert_eq!(
        context.display_locator(errors[1].identity()),
        Some("second.scoop")
    );
}

#[test]
fn unsupported_import_modifiers_remain_header_errors_after_declarations() {
    for modifier in ["internal", "private"] {
        let source = format!(
            "fun main() {{}}\n{modifier} /* trivia */\nimport lib.Item\npublic import lib.Other"
        );
        let errors = parse(&source).expect_err("late imports and unsupported modifier fail");
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert_eq!(errors[0].span, Some(span_of(&source, modifier)));
        assert_eq!(
            errors[0].message,
            "`import` headers must appear before declarations"
        );
        assert_eq!(errors[1].span, Some(span_of(&source, modifier)));
        assert_eq!(
            errors[1].message,
            format!("`{modifier} import` is not supported; imports may be ordinary or `public`")
        );
        assert_eq!(
            errors[2].message,
            "`import` headers must appear before declarations"
        );
    }
}

#[test]
fn parse_all_is_atomic_with_valid_sources_on_both_sides_of_errors() {
    let texts = [
        "package first",
        "import .bad",
        "fun main() {}",
        "import a.* as alias",
        "package last",
    ];
    let identities: Vec<_> = (0..texts.len())
        .map(|index| source_identity(&format!("src/source-{index}.scoop")))
        .collect();
    let inputs: Vec<_> = texts
        .iter()
        .enumerate()
        .map(|(index, text)| IdentifiedSourceInput::new(&identities[index], text))
        .collect();
    let errors = parse_all(NonEmptyVec::new(inputs[0], inputs[1..].to_vec()))
        .expect_err("valid subsets must not escape when another source fails");
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].identity(), &identities[1]);
    assert_eq!(errors[0].diagnostic().file, 1);
    assert_eq!(errors[0].diagnostic().span, Some(Span::new(7, 8)));
    assert_eq!(errors[1].identity(), &identities[3]);
    assert_eq!(errors[1].diagnostic().file, 3);
    assert_eq!(errors[1].diagnostic().span, Some(Span::new(11, 13)));
}

#[test]
fn duplicate_identity_diagnostic_uses_original_input_index_after_earlier_duplicates() {
    let first_identity = source_identity("src/first.scoop");
    let second_identity = source_identity("src/second.scoop");
    let first = IdentifiedSourceInput::new(&first_identity, "");
    let second = IdentifiedSourceInput::new(&second_identity, "");
    let errors = parse_all(NonEmptyVec::new(first, vec![first, second, second]))
        .expect_err("each duplicate is rejected");
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].diagnostic().file, 1);
    assert_eq!(
        errors[0].diagnostic().message,
        format!(
            "duplicate source identity {}/src/first.scoop (first used by source 0)",
            first_identity.cone()
        )
    );
    assert_eq!(errors[1].diagnostic().file, 3);
    assert_eq!(
        errors[1].diagnostic().message,
        format!(
            "duplicate source identity {}/src/second.scoop (first used by source 2)",
            second_identity.cone()
        )
    );
}

#[test]
fn parse_all_uses_only_explicit_text_and_has_no_display_locator_semantics() {
    let identity = source_identity("src/main.scoop");
    let parse_at = || {
        parse_all(NonEmptyVec::new(
            IdentifiedSourceInput::new(&identity, "fun main() {}"),
            Vec::new(),
        ))
        .expect("the locator need not exist or agree with the supplied text")
    };
    let first = parse_at();
    let fixture_directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m21-const"
    );
    assert!(
        std::path::Path::new(fixture_directory)
            .join("invalid.scoop")
            .exists()
    );
    // A neighboring source must never be discovered through the display label.
    let second = parse_at();
    assert_eq!(first, second);
    assert_eq!(first.sources().len(), 1);
    assert_eq!(
        scoop_ast::dump(first.sources().first().ast()),
        "SourceFile\n  RootPackage\n  fun main()\n"
    );
}
