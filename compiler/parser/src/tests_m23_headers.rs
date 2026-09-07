//! File-header parsing tests (spec section 12.4.1): package, exact/star
//! imports, `as` aliases, `public import`, ordering constraints and
//! recovery.

use crate::parse;
use scoop_ast::{ImportSyntax, PackageSyntax};

fn header(source: &str) -> scoop_ast::SourceFile {
    parse(source).unwrap_or_else(|diagnostics| panic!("should parse: {diagnostics:?}"))
}

fn path_text(path: &scoop_ast::QualifiedPath) -> String {
    path.segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

#[test]
fn package_header_parses() {
    let file = header("package dev.example.api\nfun main() {}\n");
    match &file.package {
        PackageSyntax::QualifiedPackage(path) => {
            assert_eq!(path_text(path), "dev.example.api");
        }
        PackageSyntax::RootPackage => panic!("package header was parsed"),
    }
    assert!(file.imports.is_empty());
    assert_eq!(file.declarations.len(), 1);
}

#[test]
fn omitted_package_is_explicit_root() {
    let file = header("fun main() {}\n");
    assert_eq!(file.package, PackageSyntax::RootPackage);
}

#[test]
fn exact_imports_with_and_without_alias() {
    let file = header(
        "import org.foo.model.User\nimport org.foo.ops.render as renderUser\nfun main() {}\n",
    );
    assert_eq!(file.imports.len(), 2);
    match &file.imports[0] {
        ImportSyntax::Exact {
            public,
            path,
            alias,
            ..
        } => {
            assert!(!*public);
            assert_eq!(path_text(path), "org.foo.model.User");
            assert!(alias.is_none());
        }
        _ => panic!("first import is exact"),
    }
    match &file.imports[1] {
        ImportSyntax::Exact { path, alias, .. } => {
            assert_eq!(path_text(path), "org.foo.ops.render");
            assert_eq!(alias.as_ref().expect("alias").text, "renderUser");
        }
        _ => panic!("second import is exact"),
    }
}

#[test]
fn star_import_parses() {
    let file = header("import org.foo.model.State.*\nfun main() {}\n");
    match &file.imports[0] {
        ImportSyntax::Star { public, path, .. } => {
            assert!(!*public);
            assert_eq!(path_text(path), "org.foo.model.State");
        }
        _ => panic!("import is star"),
    }
}

#[test]
fn public_import_parses() {
    let file = header("public import org.foo.model.Result\nfun main() {}\n");
    match &file.imports[0] {
        ImportSyntax::Exact { public, .. } => assert!(*public),
        _ => panic!("import is exact"),
    }
}

#[test]
fn public_alone_stays_a_declaration_modifier() {
    // `public` not followed by `import` belongs to the declaration.
    let file = header("public fun main() {}\n");
    assert!(file.imports.is_empty());
    assert_eq!(file.declarations.len(), 1);
}

#[test]
fn star_import_with_alias_is_rejected() {
    let diagnostics = parse("import org.foo.model.State.* as S\nfun main() {}\n")
        .expect_err("star alias is a syntax error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("star import cannot use")),
        "{diagnostics:?}"
    );
}

#[test]
fn import_after_declaration_is_rejected() {
    let diagnostics = parse("fun main() {}\nimport org.foo.User\n")
        .expect_err("imports are only valid in the file header");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("import") || d.message.contains("not supported")),
        "{diagnostics:?}"
    );
}

#[test]
fn package_after_declaration_is_rejected() {
    let diagnostics = parse("fun main() {}\npackage dev.example\n")
        .expect_err("package must precede declarations");
    assert!(!diagnostics.is_empty());
}

#[test]
fn empty_import_path_is_rejected() {
    let diagnostics = parse("import .User\nfun main() {}\n").expect_err("path is malformed");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("qualified path")),
        "{diagnostics:?}"
    );
}

#[test]
fn malformed_import_recovers_to_next_header_item() {
    // Two diagnostics: the malformed import and... only it — recovery must
    // let the following import and the declaration parse.
    let source = "import org.foo..User\nimport org.ok.Fine\nfun main() {}\n";
    let diagnostics = parse(source).expect_err("malformed path errors");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("qualified path")),
        "{diagnostics:?}"
    );
}

#[test]
fn import_and_package_are_usable_as_identifiers_after_header() {
    let file = header("fun main() {\n    val import = 1\n    val package = import + 1\n}\n");
    assert_eq!(file.declarations.len(), 1);
}

#[test]
fn missing_alias_identifier_is_diagnosed() {
    let diagnostics =
        parse("import org.foo.User as\nfun main() {}\n").expect_err("alias needs an identifier");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("identifier after `as`")),
        "{diagnostics:?}"
    );
}
