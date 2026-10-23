use scoop_ast::{
    DeclarationFragment, DeclarationHeaderKind, DeclarationNamespaces, DeclaredVisibility,
    VisibilitySyntax,
};

#[test]
fn failed_callables_keep_the_receiver_and_modifiers_decided_by_the_grammar() {
    for (source, kind) in [
        (
            "fun editing(value: ) {}",
            DeclarationHeaderKind::Function {
                is_extension: false,
                is_operator: false,
                is_infix: false,
            },
        ),
        (
            "operator fun Boolean.editing(value: ) {}",
            DeclarationHeaderKind::Function {
                is_extension: true,
                is_operator: true,
                is_infix: false,
            },
        ),
        (
            "infix fun Boolean.editing(value: ) {}",
            DeclarationHeaderKind::Function {
                is_extension: true,
                is_operator: false,
                is_infix: true,
            },
        ),
        (
            "val editing: = true",
            DeclarationHeaderKind::Property {
                is_extension: false,
            },
        ),
        (
            "val Boolean.editing: = true",
            DeclarationHeaderKind::Property { is_extension: true },
        ),
    ] {
        let parsed = scoop_parser::parse_recovering(source).unwrap();
        let header = parsed
            .declaration_data()
            .declarations
            .iter()
            .find_map(|item| match &item.value {
                DeclarationFragment::Unavailable(header) if header.name.text == "editing" => {
                    Some(header)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing header: {source}"));
        assert_eq!(header.kind, kind, "{source}");
    }
}

#[test]
fn failed_declarations_keep_grammar_names_visibility_and_namespaces() {
    for (source, namespace) in [
        (
            "private fun editing(value: ) {}",
            DeclarationNamespaces::Value,
        ),
        ("private val editing: = 1", DeclarationNamespaces::Value),
        ("private typealias editing =", DeclarationNamespaces::Type),
        (
            "private struct editing(val value: )",
            DeclarationNamespaces::Type,
        ),
        (
            "private enum editing { Case(value: ) }",
            DeclarationNamespaces::Type,
        ),
        (
            "private class editing(val value: ) {}",
            DeclarationNamespaces::Type,
        ),
        (
            "private interface editing { fun broken(value: ) }",
            DeclarationNamespaces::Type,
        ),
        (
            "private object editing { val value: = 1 }",
            DeclarationNamespaces::TypeAndValue,
        ),
        (
            "private annotation class editing(val value: )",
            DeclarationNamespaces::Type,
        ),
    ] {
        let parsed = scoop_parser::parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none(), "{source}");
        let header = parsed
            .declaration_data()
            .declarations
            .iter()
            .find_map(|item| match &item.value {
                DeclarationFragment::Unavailable(header) if header.name.text == "editing" => {
                    Some(header)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing declaration header: {source}"));
        assert_eq!(header.kind.namespaces(), namespace);
        assert!(matches!(
            header.visibility,
            VisibilitySyntax::Explicit {
                visibility: DeclaredVisibility::Private,
                ..
            }
        ));
        assert_eq!(
            &source[header.name.span.start as usize..header.name.span.end as usize],
            "editing"
        );
    }
}

#[test]
fn complete_declarations_and_signatures_do_not_get_unavailable_duplicates() {
    for source in [
        "class Complete { fun good(): Int = 1 }\nfun broken(value: ) {}",
        "fun editing(): Int { value. }\nfun broken(value: ) {}",
    ] {
        let parsed = scoop_parser::parse_recovering(source).unwrap();
        let unavailable = parsed
            .declaration_data()
            .declarations
            .iter()
            .filter_map(|item| match &item.value {
                DeclarationFragment::Unavailable(header) => Some(header.name.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(unavailable, ["broken"]);
    }
}
