use scoop_ast::{DeclarationFragment, PackageSyntax};
use scoop_parser::{ParsedFile, parse_recovering};
use scoop_syntax::SyntaxKind;

fn complete_names(parsed: &ParsedFile) -> Vec<&str> {
    parsed
        .declaration_data()
        .declarations
        .iter()
        .filter_map(|item| {
            let DeclarationFragment::Complete(_) = item.value else {
                return None;
            };
            let declaration = &parsed.declarations()[item.declaration.0];
            assert!(declaration.parent.is_none());
            Some(declaration.name.as_ref().unwrap().text.as_str())
        })
        .collect()
}

#[test]
fn failed_file_retains_real_complete_siblings_and_their_file_context() {
    let source = "package demo\nimport example.Type\nfun editing(user: User) { user. }\nclass User {}\nfun other(): Boolean = true\ntypealias Flag = Boolean\nval answer: Int = 1";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(complete_names(&parsed), ["User", "other", "Flag", "answer"]);
    let header = parsed.declaration_data().source.as_ref().unwrap();
    assert!(matches!(
        header.package,
        Some(PackageSyntax::QualifiedPackage { .. })
    ));
    assert_eq!(header.imports.len(), 1);
    assert!(header.imports_complete);
    assert!(parsed.declaration_data().declarations.iter().any(|item| {
        matches!(&item.value, DeclarationFragment::FunctionHeader(header) if header.signature.name.text == "editing")
    }));
}

#[test]
fn every_top_level_kind_maps_to_its_own_grammar_declaration() {
    let source = "fun broken() { bad. }\nstruct S()\nenum E { One }\nclass C\ninterface I {}\nobject O {}\nannotation class A\ntypealias T = Int\nval p: Int = 1\nfun f() {}";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(
        complete_names(&parsed),
        ["S", "E", "C", "I", "O", "A", "T", "p", "f"]
    );
    let kinds = parsed
        .declaration_data()
        .declarations
        .iter()
        .filter_map(|item| {
            matches!(item.value, DeclarationFragment::Complete(_))
                .then_some(parsed.declarations()[item.declaration.0].kind)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            SyntaxKind::StructDecl,
            SyntaxKind::EnumDecl,
            SyntaxKind::ClassDecl,
            SyntaxKind::InterfaceDecl,
            SyntaxKind::ObjectDecl,
            SyntaxKind::AnnotationClassDecl,
            SyntaxKind::TypeAliasDecl,
            SyntaxKind::PropertyDecl,
            SyntaxKind::FunctionDecl
        ]
    );
}

#[test]
fn member_recovery_does_not_turn_the_incomplete_owner_into_a_complete_declaration() {
    let source = "class Broken { fun bad(value: ) {}\nfun good() = 1 }\nclass Complete {}";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(complete_names(&parsed), ["Complete"]);
    let good = parsed.declaration_data().declarations.iter().find(|item| {
        matches!(&item.value, DeclarationFragment::FunctionHeader(header) if header.signature.name.text == "good")
    }).unwrap();
    let parent = parsed.declarations()[good.declaration.0].parent.unwrap();
    assert_eq!(
        parsed.declarations()[parent.0].name.as_ref().unwrap().text,
        "Broken"
    );
}

#[test]
fn lexical_errors_prevent_complete_declaration_fragments() {
    let parsed = parse_recovering("val broken: String = \"\\q\"\nfun good() {}").unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(complete_names(&parsed), ["good"]);
}

#[test]
fn package_errors_are_not_reported_as_a_known_root_package() {
    for source in [
        "package\nfun good() {}",
        "package first\npackage second\nfun good() {}",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none());
        assert_eq!(complete_names(&parsed), ["good"]);
        assert!(
            parsed
                .declaration_data()
                .source
                .as_ref()
                .unwrap()
                .package
                .is_none()
        );
    }
}

#[test]
fn incomplete_imports_remain_distinct_from_a_valid_empty_import_list() {
    for source in [
        "import .\nfun good() {}",
        "private import example.Type\nfun good() {}",
        "fun good() {}\nimport example.Type",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none());
        assert_eq!(complete_names(&parsed), ["good"]);
        let header = parsed.declaration_data().source.as_ref().unwrap();
        assert_eq!(header.package, Some(PackageSyntax::RootPackage));
        assert!(!header.imports_complete);
    }
    let parsed = parse_recovering("fun bad() { value. }\nfun good() {}").unwrap();
    let header = parsed.declaration_data().source.as_ref().unwrap();
    assert!(header.imports_complete);
    assert!(header.imports.is_empty());
}

#[test]
fn complete_files_use_the_strict_ast_without_copying_complete_declarations() {
    let parsed =
        parse_recovering("class User { fun name(): String = \"user\" }\nfun use(value: User) {}")
            .unwrap();
    assert!(parsed.ast().is_some());
    assert!(parsed.declaration_data().source.is_none());
    assert!(complete_names(&parsed).is_empty());
}

#[test]
fn nested_default_declarations_do_not_take_the_top_level_projection() {
    let source = "fun factory(value: Int = { fun nested() = 1\nnested() }()): Int = value\nfun broken() { bad. }";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(complete_names(&parsed), ["factory"]);
}
