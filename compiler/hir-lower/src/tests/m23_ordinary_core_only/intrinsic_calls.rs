use super::*;
use scoop_ast as ast;
use scoop_hir as hir;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-imported-core-const")
            .join(name),
    )
    .unwrap()
}

fn const_integer(output: &hir::DependencyHirOutput, name: &str) -> hir::HirIntegerConstant {
    let (_, property) = output
        .output()
        .export
        .properties
        .iter()
        .find(|(_, property)| property.name == name)
        .unwrap();
    let hir::PropertyRepresentation::Const {
        value: hir::ConstPropertyValue::Integer(value),
    } = property.representation
    else {
        panic!("expected an integer constant")
    };
    value
}

#[test]
fn imported_intrinsic_methods_infix_and_static_values_use_source_contracts() {
    constants::with_input(&fixture("methods.scoop"), |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input).unwrap();
        for (name, value) in [
            ("compared", hir::HirIntegerConstant::Signed64(1)),
            ("quotient", hir::HirIntegerConstant::Signed32(5)),
            ("remainder", hir::HirIntegerConstant::Signed32(2)),
            ("converted", hir::HirIntegerConstant::Unsigned16(5)),
            ("inverted", hir::HirIntegerConstant::Signed8(255)),
            ("shifted", hir::HirIntegerConstant::Unsigned32(12)),
        ] {
            assert_eq!(const_integer(&output, name), value, "{name}");
        }
        let export = output.output().export.module();
        let (_, global) = export
            .globals
            .iter()
            .find(|(_, global)| export.properties[global.property].name == "staticValue")
            .unwrap();
        assert!(matches!(
            global.storage,
            hir::GlobalStorage::Managed {
                state: hir::HirStaticInitialState::EncodedStaticValue {
                    payload: hir::HirConstantImage::Integer(hir::HirIntegerConstant::Signed64(1)),
                }
            }
        ));
        assert!(
            export
                .functions
                .iter()
                .all(|(_, function)| !matches!(function.kind, hir::FunctionKind::Intrinsic(_)))
        );
        assert!(output.imported_dependencies().is_empty());
    });
}

fn assert_error(input: &CurrentConeSources<'_, '_>, source: &str, expression: &str, message: &str) {
    let diagnostics = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
        .err()
        .expect("invalid intrinsic source call");
    let start = source.find(expression).unwrap() as u32;
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == message
                && diagnostic.file == 0
                && diagnostic.span
                    == Some(ast::Span {
                        start,
                        end: start + expression.len() as u32,
                    })),
        "{diagnostics:?}"
    );
}

#[test]
fn imported_intrinsic_method_failures_keep_precise_source_diagnostics() {
    for (name, expression, message) in [
        (
            "wrong-argument.scoop",
            "7.compareTo(rhs = 3)",
            "const integer intrinsic arguments do not match the exact typed core declaration",
        ),
        (
            "non-infix.scoop",
            "plus",
            "const infix call did not resolve to a typed core integer infix intrinsic",
        ),
        (
            "method-division-zero.scoop",
            "12.div(other = 0)",
            "division by zero in const initializer",
        ),
        (
            "conversion-argument.scoop",
            "3L.toInt32(1)",
            "const integer conversion arguments do not match the exact typed core declaration",
        ),
    ] {
        let source = fixture(name);
        constants::with_input(&source, |input| {
            assert_error(input, &source, expression, message)
        });
    }
}

#[test]
fn imported_intrinsics_keep_edited_parameter_names_and_declared_method_names() {
    let mut source = super::super::complete_core_file();
    let method = source
        .declarations
        .iter_mut()
        .find_map(|declaration| {
            let ast::Decl::Struct(owner) = declaration else {
                return None;
            };
            if owner.name.text != "Int" {
                return None;
            }
            owner.members.iter_mut().find_map(|member| match member {
                ast::StructMember::Function(method) if method.name.text == "shl" => Some(method),
                _ => None,
            })
        })
        .unwrap();
    method.params[0].name.text = "distance".into();
    let core = support::trusted_core_from_source(source, "", None);
    let run = |name: &str, check: &dyn Fn(&CurrentConeSources<'_, '_>, &str)| {
        let source = fixture(name);
        let parsed = support::parsed_ordinary_text(&source);
        let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
        let world = core.world(parsed.cone());
        let input = CurrentConeSources::try_new(&parsed, protocols, &world).unwrap();
        check(&input, &source);
    };
    run("edited-parameter.scoop", &|input, _| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input).unwrap();
        for name in ["named", "shifted"] {
            assert_eq!(
                const_integer(&output, name),
                hir::HirIntegerConstant::Signed32(12)
            );
        }
    });
    run("unknown-method.scoop", &|input, source| {
        assert_error(
            input,
            source,
            "shiftBits",
            "const initializer did not resolve to the exact typed core integer intrinsic",
        )
    });
    run("edited-parameter-old-name.scoop", &|input, source| {
        assert_error(
            input,
            source,
            "3.shl(count = 2L)",
            "const integer intrinsic arguments do not match the exact typed core declaration",
        )
    });
}
