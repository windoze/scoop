use scoop_ast::{DeclarationFragment as Fragment, RecoveredNominal as Nominal};
use scoop_parser::parse_recovering;
use scoop_syntax::SyntaxKind;

#[test]
fn completed_interface_blocks_survive_recovery_of_a_sibling_method() {
    for header in [
        "class Holder<T> {",
        "struct Holder<T>(val item: T) {",
        "enum Holder<T> { Present(T)\n",
    ] {
        let text = format!(
            "{header}\n\
             override interface Flag where T : ref {{\n\
                 public override fun enabled(): Boolean = true\n\
             }}\n\
             fun editing(value: T) {{ value. }}\n\
             }}"
        );
        let parsed = parse_recovering(&text).unwrap();
        assert!(parsed.ast().is_none());
        assert_eq!(parsed.syntax().to_string(), text);
        assert_eq!(
            parsed
                .syntax()
                .descendants()
                .filter(|node| node.kind() == SyntaxKind::InterfaceImplementation)
                .count(),
            1
        );
        let nominal = parsed
            .declaration_data()
            .declarations
            .iter()
            .find_map(|item| match &item.value {
                Fragment::Nominal(nominal) => Some(nominal),
                _ => None,
            })
            .unwrap_or_else(|| panic!("the complete implementation remains in {text}"));
        let (implementation, recovered_methods) = match nominal {
            Nominal::Class(owner) => (
                owner.members.iter().find_map(|member| match member {
                    scoop_ast::ClassMember::InterfaceImplementation(block) => Some(block.as_ref()),
                    _ => None,
                }),
                &owner.recovery.methods,
            ),
            Nominal::Struct(owner) => (
                owner.members.iter().find_map(|member| match member {
                    scoop_ast::StructMember::InterfaceImplementation(block) => Some(block.as_ref()),
                    _ => None,
                }),
                &owner.recovery.methods,
            ),
            Nominal::Enum(owner) => (
                owner.interface_implementations.first(),
                &owner.recovery.methods,
            ),
            _ => panic!("the original nominal kind is preserved"),
        };
        let implementation = implementation.unwrap();
        assert_eq!(implementation.where_clause.constraints.len(), 1);
        assert_eq!(
            implementation.functions().next().unwrap().name.text,
            "enabled"
        );
        assert_eq!(recovered_methods.len(), 1);
        assert_eq!(recovered_methods[0].header.signature.name.text, "editing");
    }
}

#[test]
fn malformed_interface_blocks_do_not_publish_unconditional_members() {
    for block in [
        "override interface Flag where { public override fun enabled(): Boolean = true }",
        "override interface Flag where T : ref { public override fun enabled(value: ) {} }",
    ] {
        let text = format!("class Holder<T> {{ {block}\nfun editing(value: T) {{ value. }} }}");
        let parsed = parse_recovering(&text).unwrap();
        assert!(parsed.ast().is_none());
        assert_eq!(parsed.syntax().to_string(), text);
        assert!(
            !parsed
                .declaration_data()
                .declarations
                .iter()
                .any(|item| matches!(item.value, Fragment::Nominal(_))),
            "{text}"
        );
        assert!(parsed.declaration_data().declarations.iter().any(|item| {
            matches!(&item.value, Fragment::Unavailable(header) if header.name.text == "Holder")
        }));
    }
}
