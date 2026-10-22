use super::*;

mod recovered;
mod unavailable_signatures;

#[test]
fn empty_input_is_diagnosed_before_selecting_a_current_cone() {
    let Err(diagnostics) = Lowerer::new().run(&[], None, None) else {
        panic!("an empty source set cannot compile");
    };
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message, "no source files to compile");
}

fn input(source: &str) -> (Lowerer, Vec<ast::SourceFile>) {
    let files = vec![
        crate::tests::core_file(),
        scoop_parser::parse(source).unwrap(),
    ];
    (source_lowerer(source), files)
}

fn source_lowerer(source: &str) -> Lowerer {
    source_lowerer_files(&[(source, scoop_identity::SourceRole::Production)])
}

fn source_lowerer_files(user_sources: &[(&str, scoop_identity::SourceRole)]) -> Lowerer {
    let mut sources = vec![SourceProvider {
        provider: hir::IntrinsicProviderId::from_raw(0),
        kind: SourceKind::Core,
        role: scoop_identity::SourceRole::Production,
        identity: crate::tests::core_source_identity("src/core.scoop"),
        name: "core.scoop".into(),
        source: String::new(),
    }];
    sources.extend(
        user_sources
            .iter()
            .enumerate()
            .map(|(index, &(source, role))| SourceProvider {
                provider: hir::IntrinsicProviderId::from_raw(1),
                kind: SourceKind::CurrentUnit,
                role,
                identity: crate::tests::test_source_identity(&format!("src/user{index}.scoop")),
                name: format!("user{index}.scoop"),
                source: source.into(),
            }),
    );
    let lowerer =
        Lowerer::new().with_intrinsic_sources(sources, IntrinsicDeclarationPolicy::CoreOnly);
    let dependencies = hir::ImportedDependencySelectionPlan::empty(lowerer.current_cone());
    lowerer.with_imported_dependencies(dependencies)
}

#[test]
fn a_header_only_result_query_does_not_publish_the_pending_storage_type() {
    for (source, pending) in [("fun subject() = true", true), ("fun subject() {}", false)] {
        let (mut lowerer, files) = input(source);
        let declarations = lowerer.collect_source_declarations(&files, None).unwrap();
        let &(function, declaration, file) = declarations
            .pending_functions
            .iter()
            .find(|(_, declaration, _)| declaration.name.text == "subject")
            .unwrap();
        let parsed = scoop_parser::parse_recovering(source).unwrap();
        let header = parsed
            .declaration_data()
            .declarations
            .iter()
            .find_map(|item| match &item.value {
                ast::DeclarationFragment::FunctionHeader(header) => Some(header.as_ref()),
                ast::DeclarationFragment::Complete(_)
                | ast::DeclarationFragment::Unavailable(_) => None,
            })
            .unwrap();
        lowerer.current_file = file;
        lowerer.resolve_function_header(
            function,
            ast::FunctionHeaderRef {
                signature: &header.signature,
                body_form: header.body_form,
                span: declaration.span,
            },
        );
        assert_eq!(lowerer.signatures[&function].result_pending, pending);
        assert_eq!(
            lowerer.infer_function_result(function),
            (!pending).then_some(lowerer.unit)
        );
    }
}

#[test]
fn explicit_signatures_are_available_before_erroneous_bodies_are_checked() {
    let (mut lowerer, files) = input(
        "fun caller(): Boolean = known(true)\nfun known(value: Boolean): Boolean { missingBodyName }",
    );
    let declarations = lowerer.collect_source_declarations(&files, None).unwrap();
    lowerer.resolve_source_signatures(&declarations).unwrap();
    assert!(lowerer.diagnostics.is_empty(), "{:?}", lowerer.diagnostics);
    let known = declarations
        .pending_functions
        .iter()
        .find(|(_, declaration, _)| declaration.name.text == "known")
        .unwrap()
        .0;
    let signature = &lowerer.signatures[&known];
    assert_eq!(signature.return_ty, lowerer.boolean);
    assert_eq!(signature.params[0].ty, lowerer.boolean);
    assert!(!signature.result_pending);
    let core = lowerer.complete_declaration_bodies(&declarations).unwrap();
    assert!(lowerer.diagnostics.is_empty(), "{:?}", lowerer.diagnostics);
    lowerer.lower_source_bodies(&declarations, core.type_reference);
    assert!(
        lowerer
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("missingBodyName"))
    );
}

#[test]
fn inferred_result_is_pending_until_its_actual_body_completes() {
    let (mut lowerer, files) =
        input("fun inferred() = missingReturnValue\nfun other(): Boolean = true");
    let declarations = lowerer.collect_source_declarations(&files, None).unwrap();
    lowerer.resolve_source_signatures(&declarations).unwrap();
    assert!(lowerer.diagnostics.is_empty(), "{:?}", lowerer.diagnostics);
    let inferred = declarations
        .pending_functions
        .iter()
        .find(|(_, declaration, _)| declaration.name.text == "inferred")
        .unwrap()
        .0;
    assert!(lowerer.signatures[&inferred].result_pending);
    let Err(diagnostics) = lowerer.complete_declaration_bodies(&declarations) else {
        panic!("an unresolved inferred result must keep declaration completion from succeeding");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("missingReturnValue"))
    );
    assert!(lowerer.signatures[&inferred].result_pending);
}
