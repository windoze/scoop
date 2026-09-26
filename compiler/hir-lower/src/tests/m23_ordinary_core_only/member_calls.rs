use super::*;
use scoop_hir as hir;

#[test]
fn imported_core_members_normalize_through_shared_source_candidates() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-imported-core-members/methods.scoop"
    ));
    constants::with_input(source, |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
            .unwrap_or_else(|errors| panic!("{errors:#?}"));
        let export = output.output().export.module();
        assert!(
            export
                .functions
                .iter()
                .all(|(_, function)| !matches!(function.kind, hir::FunctionKind::Intrinsic(_)))
        );
        assert!(output.imported_dependencies().is_empty());
        let dump = hir::dump(&output.output().export);
        assert!(
            dump.contains("IntegerOperation int.compare_to <no-gc> : Long"),
            "{dump}"
        );
        assert!(
            dump.contains("IntegerOperation int8.add <no-gc> : Int8"),
            "{dump}"
        );
        assert!(
            dump.contains("IntegerConversion Long -> Int <no-gc> : Int"),
            "{dump}"
        );
        assert!(!dump.contains("ImportedDependencyCall"));
    });
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-imported-core-members")
            .join(name),
    )
    .unwrap()
}

fn assert_error(input: &CurrentConeSources<'_, '_>, source: &str, expression: &str, message: &str) {
    let errors = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
        .err()
        .expect("invalid member call must fail in HIR");
    let start = source.find(expression).unwrap() as u32;
    assert!(
        errors.iter().any(|error| error.message == message
            && error.file == 0
            && error.span == Some(scoop_ast::Span::new(start, start + expression.len() as u32))),
        "{errors:#?}"
    );
}

#[test]
fn imported_core_members_preserve_source_argument_diagnostics() {
    for (name, expression, message) in [
        (
            "wrong-name.scoop",
            "value.compareTo(rhs = 3)",
            "dependency function `compareTo` has no parameter named `rhs`",
        ),
        (
            "wrong-type.scoop",
            "\"text\"",
            "dependency function argument must be of type Int, found String",
        ),
        (
            "non-infix.scoop",
            "plus",
            "type `Int` has no infix callable `plus`",
        ),
        (
            "conversion-argument.scoop",
            "value.toInt32(1)",
            "dependency function `toInt32` expects 0 argument(s), but 1 were supplied",
        ),
        (
            "managed.scoop",
            "value.div(2)",
            "SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED: dependency callable requires layout/ABI capability from M23-6",
        ),
    ] {
        let source = fixture(name);
        constants::with_input(&source, |input| {
            assert_error(input, &source, expression, message)
        });
    }
}

#[test]
fn imported_member_named_arguments_follow_the_edited_source_declaration() {
    let mut source = super::super::complete_core_file();
    let method = source
        .declarations
        .iter_mut()
        .find_map(|declaration| {
            let scoop_ast::Decl::Struct(owner) = declaration else {
                return None;
            };
            if owner.name.text != "Int" {
                return None;
            }
            owner.members.iter_mut().find_map(|member| match member {
                scoop_ast::StructMember::Function(method) if method.name.text == "shl" => {
                    Some(method)
                }
                _ => None,
            })
        })
        .unwrap();
    method.params[0].name.text = "distance".into();
    let core = support::trusted_core_from_source(source, "");
    for name in ["edited-parameter.scoop", "edited-old-name.scoop"] {
        let source = fixture(name);
        let parsed = support::parsed_ordinary_text(&source);
        let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
        let world = core.world(parsed.cone());
        let input = CurrentConeSources::try_new(&parsed, protocols, &world).unwrap();
        if name == "edited-parameter.scoop" {
            let output =
                lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).unwrap();
            assert!(
                hir::dump(&output.output().export)
                    .contains("IntegerOperation int.shl <no-gc> : Int")
            );
        } else {
            assert_error(
                &input,
                &source,
                "value.shl(count = 2L)",
                "dependency function `shl` has no parameter named `count`",
            );
        }
    }
}
