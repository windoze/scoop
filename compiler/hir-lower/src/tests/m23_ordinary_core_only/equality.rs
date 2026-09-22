use super::*;
use scoop_hir as hir;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-imported-core-members")
            .join(name),
    )
    .unwrap()
}

#[test]
fn imported_integer_equality_uses_typed_members_and_single_operand_evaluation() {
    constants::with_input(&fixture("equality.scoop"), |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
            .unwrap_or_else(|errors| panic!("{errors:#?}"));
        let dump = hir::dump(&output.output().export);
        for kind in [
            "int8", "int16", "int", "long", "uint8", "uint16", "uint", "ulong",
        ] {
            assert!(
                dump.contains(&format!("IntegerOperation {kind}.equals <no-gc> : Boolean")),
                "{dump}"
            );
        }
        assert_eq!(dump.matches("Call leftOperand : Int").count(), 1, "{dump}");
        assert_eq!(
            dump.matches("Call middleOperand : Int").count(),
            1,
            "{dump}"
        );
        assert_eq!(dump.matches("Call rightOperand : Int").count(), 1, "{dump}");
        assert!(
            dump.find("Call leftOperand : Int").unwrap()
                < dump.find("Call middleOperand : Int").unwrap()
        );
        assert!(
            dump.find("Call middleOperand : Int").unwrap()
                < dump.find("Call rightOperand : Int").unwrap()
        );
        assert!(
            dump.find("Call leftOperand : Int").unwrap()
                < dump.find("Call rightOperand : Int").unwrap()
        );
        assert!(output.imported_dependencies().is_empty());
    });
}

#[test]
fn imported_integer_literal_patterns_keep_complete_equality_plans() {
    constants::with_input(&fixture("equality-patterns.scoop"), |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
            .unwrap_or_else(|errors| panic!("{errors:#?}"));
        let export = output.output().export.module();
        let mut kinds = std::collections::BTreeSet::new();
        for (_, function) in export
            .functions
            .iter()
            .filter(|(_, function)| function.name.starts_with("pattern"))
        {
            let hir::FunctionKind::User(body) = &function.kind else {
                panic!("ordinary function body")
            };
            let when = body
                .statements
                .iter()
                .find_map(|statement| match &statement.kind {
                    hir::StatementKind::When(when) => Some(when),
                    _ => None,
                })
                .expect("pattern function contains a when");
            let hir::Pattern::Literal {
                value,
                equality: hir::LiteralPatternEquality::Integer { kind },
                ..
            } = &when.arms[0].pattern
            else {
                panic!("integer literal pattern carries its normalized equality");
            };
            assert_eq!(export.types[value.ty], hir::Type::Integer(*kind));
            kinds.insert(*kind);
        }
        assert_eq!(kinds, hir::IntegerKind::ALL.into_iter().collect());
        assert!(output.imported_dependencies().is_empty());
    });
}

#[test]
fn imported_equality_rejects_mixed_types_and_unrelated_extensions() {
    for (name, expression, message) in [
        (
            "equality-width.scoop",
            "right",
            "dependency function argument must be of type Int, found Long",
        ),
        (
            "equality-signedness.scoop",
            "right",
            "dependency function argument must be of type Int, found UInt",
        ),
        (
            "equality-extension.scoop",
            "right",
            "dependency function argument must be of type Int, found Long",
        ),
        (
            "equality-pattern-overflow.scoop",
            "128",
            "integer literal `128` is not representable as Int8",
        ),
    ] {
        let source = fixture(name);
        constants::with_input(&source, |input| {
            let errors = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
                .err()
                .expect("invalid equality must fail in HIR");
            let start = source.rfind(expression).unwrap() as u32;
            assert!(
                errors.iter().any(|error| error.message == message
                    && error.file == 0
                    && error.span
                        == Some(scoop_ast::Span::new(start, start + expression.len() as u32))),
                "{errors:#?}"
            );
        });
    }
}
