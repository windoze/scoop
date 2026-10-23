use super::*;

#[test]
fn incomplete_declarations_only_block_roles_they_can_participate_in() {
    for source in [
        "fun Boolean.choose(value: ) {}\nfun choose(value: Boolean): Boolean = true\nfun healthy(): Boolean = choose(true)",
        "fun choose(value: ) {}\nfun Boolean.choose(value: Boolean): Boolean = true\nfun healthy(): Boolean = true.choose(true)",
        "fun choose(value: ) {}\nval choose: Boolean = true\nfun healthy(): Boolean = choose\nclass Reader { fun read(): Boolean = choose }",
        "fun Boolean.choose(value: ) {}\nval choose: Boolean = true\nfun healthy(): Boolean = choose",
        "struct Value(val data: Boolean)\nfun Value.invoke(value: ) {}\noperator fun Value.invoke(): Boolean = true\nval callable: Value = Value(true)\nfun healthy(): Boolean = callable()",
        "struct Value(val data: Int)\nfun Value.plus(value: ) {}\noperator fun Value.plus(value: Value): Value = this\nfun healthy(): Value = Value(1) + Value(2)",
        "fun Boolean.choose(value: ) {}\ninfix fun Boolean.choose(value: Boolean): Boolean = true\nfun healthy(): Boolean = true choose true",
    ] {
        let diagnostics = body_diagnostics(source);
        assert!(diagnostics.is_empty(), "{source}\n{diagnostics:?}");
    }
}

#[test]
fn incomplete_operator_roles_block_direct_and_property_invocations() {
    for (declarations, call, blocked_name) in [
        (
            "operator fun Value.plus(value: ): Value = this\noperator fun Value.plus(value: Value): Value = this",
            "Value(true) + Value(true)",
            "plus",
        ),
        (
            "operator fun Value.invoke(value: ): Boolean = true\noperator fun Value.invoke(): Boolean = true\nval callable: Value = Value(true)",
            "callable()",
            "invoke",
        ),
        (
            "operator fun Value.invoke(value: ): Boolean = true\noperator fun Value.invoke(): Boolean = true\nclass Holder(val callable: Value)",
            "Holder(Value(true)).callable()",
            "invoke",
        ),
        (
            "operator fun Value.invoke(value: ): Boolean = true\noperator fun Value.invoke(): Boolean = true\nval Boolean.callable: Value get() = Value(true)",
            "true.callable()",
            "invoke",
        ),
    ] {
        let source =
            format!("struct Value(val data: Boolean)\n{declarations}\nfun caller() {{ {call} }}");
        let diagnostics = body_diagnostics(&source);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.message
                == format!("declaration `{blocked_name}` is incomplete")
                && diagnostic.notes.len() == 1),
            "{source}\n{diagnostics:?}"
        );
    }
}

fn body_diagnostics(source: &str) -> Vec<ast::Diagnostic> {
    let core = crate::tests::core_file();
    let parsed = scoop_parser::parse_recovering(source).unwrap();
    let mut lowerer = source_lowerer(source);
    let declarations = collect(&mut lowerer, &core, source, &parsed);
    lowerer.resolve_source_signatures(&declarations).unwrap();
    let protocols = lowerer.complete_declaration_bodies(&declarations).unwrap();
    lowerer.lower_source_bodies(&declarations, protocols.type_reference);
    lowerer.diagnostics
}

#[test]
fn incomplete_names_respect_file_visibility_packages_and_test_scope() {
    use scoop_identity::SourceRole::{Production, Test};
    for (source, role) in [
        ("package api\nprivate fun select(value: ) {}", Production),
        ("package elsewhere\nfun select(value: ) {}", Production),
        ("package api\nfun select(value: ) {}", Test),
    ] {
        let consumer = "package api\nfun select(value: Boolean): Boolean = true\n\
            fun healthy(): Boolean = select(true)";
        let core = crate::tests::core_file();
        let parsed = scoop_parser::parse_recovering(source).unwrap();
        let parsed_consumer = scoop_parser::parse_recovering(consumer).unwrap();
        let mut lowerer = source_lowerer_files(&[(source, role), (consumer, Production)]);
        let declarations = collect_files(
            &mut lowerer,
            &core,
            &[(source, &parsed), (consumer, &parsed_consumer)],
        );
        lowerer.resolve_source_signatures(&declarations).unwrap();
        let protocols = lowerer.complete_declaration_bodies(&declarations).unwrap();
        lowerer.lower_source_bodies(&declarations, protocols.type_reference);
        assert!(
            lowerer.diagnostics.is_empty(),
            "{source}\n{:?}",
            lowerer.diagnostics
        );
    }
}

#[test]
fn incomplete_overload_blocks_calls_references_and_alias_imports() {
    for (declarations, use_site, blocked_name) in [
        (
            "fun select(value: ) {}\nfun select(value: Boolean): Boolean = true",
            "select(true)",
            "select",
        ),
        (
            "fun select(value: ) {}\nfun select(value: Boolean): Boolean = true",
            "val selected: (Boolean) -> Boolean = ::select; selected(true)",
            "select",
        ),
        (
            "package api\nimport api.select as choose\nfun select(value: ) {}\n\
             fun select(value: Boolean): Boolean = true",
            "choose(true)",
            "choose",
        ),
        (
            "fun Boolean.choose(value: ): Boolean = true\n\
             fun Boolean.choose(value: Boolean): Boolean = true",
            "true.choose(true)",
            "choose",
        ),
    ] {
        let source =
            format!("{declarations}\nfun caller() {{ {use_site} }}\nfun healthy(): Boolean = true");
        let core = crate::tests::core_file();
        let parsed = scoop_parser::parse_recovering(&source).unwrap();
        let mut lowerer = source_lowerer(&source);
        let declarations = collect(&mut lowerer, &core, &source, &parsed);
        lowerer.resolve_source_signatures(&declarations).unwrap();
        let protocols = lowerer.complete_declaration_bodies(&declarations).unwrap();
        lowerer.lower_source_bodies(&declarations, protocols.type_reference);
        assert!(
            lowerer.diagnostics.iter().any(|diagnostic| {
                diagnostic.message == format!("declaration `{blocked_name}` is incomplete")
                    && diagnostic.notes.len() == 1
            }),
            "{source}\n{:?}",
            lowerer.diagnostics
        );
        let healthy = declarations
            .function_headers
            .iter()
            .find(|(_, header, _)| header.name.text == "healthy")
            .unwrap()
            .0;
        assert_eq!(lowerer.signatures[&healthy].return_ty, lowerer.boolean);
        assert!(!lowerer.signatures[&healthy].result_pending);
    }
}

#[test]
fn incomplete_type_blocks_core_fallback_without_hiding_a_value() {
    use crate::imports::lookup::LookupResult;
    let source = "typealias Boolean =\nval Boolean: Int = 1\nfun healthy(): Int = Boolean\n\
        class Reader { fun read(): Int = Boolean }";
    let core = crate::tests::core_file();
    let parsed = scoop_parser::parse_recovering(source).unwrap();
    let mut lowerer = source_lowerer(source);
    let declarations = collect(&mut lowerer, &core, source, &parsed);
    lowerer.resolve_source_signatures(&declarations).unwrap();
    lowerer.current_file = 1;
    assert!(matches!(
        lowerer.lookup_type("Boolean"),
        LookupResult::Unavailable(_)
    ));
    let protocols = lowerer.complete_declaration_bodies(&declarations).unwrap();
    lowerer.lower_source_bodies(&declarations, protocols.type_reference);
    assert!(lowerer.diagnostics.is_empty(), "{:?}", lowerer.diagnostics);
}

#[test]
fn a_lexical_parameter_shadows_an_incomplete_top_level_declaration() {
    let source = "fun select(value: ) {}\nfun healthy(select: Boolean): Boolean = select";
    let core = crate::tests::core_file();
    let parsed = scoop_parser::parse_recovering(source).unwrap();
    let mut lowerer = source_lowerer(source);
    let declarations = collect(&mut lowerer, &core, source, &parsed);
    lowerer.resolve_source_signatures(&declarations).unwrap();
    let protocols = lowerer.complete_declaration_bodies(&declarations).unwrap();
    lowerer.lower_source_bodies(&declarations, protocols.type_reference);
    assert!(lowerer.diagnostics.is_empty(), "{:?}", lowerer.diagnostics);
}

#[test]
fn higher_priority_import_can_resolve_before_an_incomplete_current_package_name() {
    let source = "package api\nimport api.Value as Choice\ntypealias Choice =\nstruct Value(val data: Int)\nfun healthy(value: Choice) {}";
    let core = crate::tests::core_file();
    let parsed = scoop_parser::parse_recovering(source).unwrap();
    let mut lowerer = source_lowerer(source);
    let declarations = collect(&mut lowerer, &core, source, &parsed);
    lowerer.resolve_source_signatures(&declarations).unwrap();
    assert!(lowerer.diagnostics.is_empty(), "{:?}", lowerer.diagnostics);
}
