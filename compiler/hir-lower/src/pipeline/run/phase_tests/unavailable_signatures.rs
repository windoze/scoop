use super::*;

#[test]
fn failed_signatures_do_not_publish_a_result() {
    for declaration in [
        "fun damaged(value: Missing): Boolean = true",
        "fun damaged(value: Boolean): Missing = true",
        "fun <T: Missing> damaged(value: T): Boolean = true",
        "fun Missing.damaged(value: Boolean): Boolean = true",
    ] {
        let source = format!("{declaration}\nfun healthy(value: Boolean): Boolean = value");
        let (mut lowerer, files) = input(&source);
        let declarations = lowerer.collect_source_declarations(&files, None).unwrap();
        lowerer.resolve_source_signatures(&declarations).unwrap();
        assert!(
            lowerer
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("Missing")),
            "{:?}",
            lowerer.diagnostics
        );
        for &(function, header, _) in &declarations.function_headers {
            match header.name.text.as_str() {
                "damaged" => {
                    assert!(
                        lowerer
                            .declaration_surface
                            .unavailable_signature(function)
                            .is_some()
                    );
                    assert!(lowerer.infer_function_result(function).is_none());
                    assert_eq!(
                        lowerer
                            .declaration_surface
                            .unavailable_signature(function)
                            .unwrap()
                            .name_span,
                        header.name.span
                    );
                }
                "healthy" => {
                    assert!(
                        lowerer
                            .declaration_surface
                            .unavailable_signature(function)
                            .is_none()
                    );
                    assert_eq!(
                        lowerer.infer_function_result(function),
                        Some(lowerer.boolean)
                    );
                }
                _ => {}
            }
        }
    }
}

#[test]
fn a_failed_signature_blocks_a_relevant_overload_or_member_layer() {
    for (declarations, call) in [
        (
            "fun select(value: Missing): Boolean = true\nfun select(value: Boolean): Boolean = true",
            "select(true)",
        ),
        (
            "fun select(value: Boolean): Missing = true\nfun select(): Boolean = true",
            "select()",
        ),
        (
            "fun Missing.select(value: Boolean): Boolean = true\nfun Boolean.select(value: Boolean): Boolean = true",
            "true.select(true)",
        ),
        (
            "class Value { fun select(value: Missing): Boolean = true }\nfun Value.select(value: Boolean): Boolean = true",
            "Value().select(true)",
        ),
        (
            "class Value { operator fun plus(value: Missing): Boolean = true }\noperator fun Value.plus(value: Value): Boolean = true",
            "Value() + Value()",
        ),
        (
            "class Value { operator fun invoke(value: Missing): Boolean = true }\noperator fun Value.invoke(): Boolean = true\nval value: Value = Value()",
            "value()",
        ),
        (
            "fun select(value: Missing): Boolean = true\nfun select(value: Boolean): Boolean = true",
            "val selected: (Boolean) -> Boolean = ::select",
        ),
    ] {
        let source =
            format!("{declarations}\nfun caller() {{ {call} }}\nfun healthy(): Boolean = true");
        let (mut lowerer, files) = input(&source);
        let declarations = lowerer.collect_source_declarations(&files, None).unwrap();
        lowerer.resolve_source_signatures(&declarations).unwrap();
        let core = lowerer.complete_declaration_bodies(&declarations).unwrap();
        let before = lowerer.diagnostics.len();
        lowerer.lower_source_bodies(&declarations, core.type_reference);
        assert!(
            lowerer.diagnostics[before..].iter().any(|diagnostic| {
                (diagnostic.message.contains("incomplete")
                    || diagnostic.message.contains("unavailable"))
                    && diagnostic.notes.len() == 1
            }),
            "{source}\n{:?}",
            lowerer.diagnostics
        );
        assert!(
            !lowerer
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("same signature")),
            "{source}\n{:?}",
            lowerer.diagnostics
        );
    }
}

#[test]
fn a_failed_nonvalue_signature_does_not_hide_an_ordinary_value() {
    let (mut lowerer, files) = input(
        "fun value(arg: Missing): Boolean = true\nval value: Boolean = true\nfun healthy(): Boolean = value\nclass Reader { fun read(): Boolean = value }",
    );
    let declarations = lowerer.collect_source_declarations(&files, None).unwrap();
    lowerer.resolve_source_signatures(&declarations).unwrap();
    let core = lowerer.complete_declaration_bodies(&declarations).unwrap();
    let before = lowerer.diagnostics.len();
    lowerer.lower_source_bodies(&declarations, core.type_reference);
    assert_eq!(
        lowerer.diagnostics.len(),
        before,
        "{:?}",
        lowerer.diagnostics
    );
}
