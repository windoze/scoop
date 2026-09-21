use super::*;
use scoop_ast as ast;
use scoop_hir as hir;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-imported-core-const/operators.scoop"
));

fn with_input<R>(source: &str, run: impl FnOnce(&OrdinarySources<'_, '_>) -> R) -> R {
    let core = trusted_core();
    let identity = crate::tests::test_source_identity("src/main.scoop");
    let sources = ast::CurrentConeParsedSources::try_new(
        ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
            ast::IdentifiedParsedSource::new(
                identity.clone(),
                scoop_parser::parse(source).unwrap(),
            ),
            vec![],
        ))
        .unwrap(),
        ast::NonEmptyVec::new(
            ast::CurrentSourceText::new(identity.clone(), source.into()),
            vec![],
        ),
        ast::NonEmptyVec::new(
            ast::CurrentSourceDiagnosticContext::new(identity, "src/main.scoop".into()),
            vec![],
        ),
    )
    .unwrap();
    let protocols = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let world = core.world(sources.cone());
    let input = OrdinarySources::try_new(&sources, protocols, &world).unwrap();
    run(&input)
}

#[test]
fn imported_core_integer_protocols_fold_const_and_static_operators() {
    with_input(SOURCE, |input| {
        let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, input).unwrap();
        let export = output.output().export.module();
        let value = |name: &str| {
            let (_, property) = export
                .properties
                .iter()
                .find(|(_, property)| property.name == name)
                .unwrap();
            let hir::PropertyRepresentation::Const { value } = &property.representation else {
                panic!("const property");
            };
            value
        };
        for (name, expected) in [
            ("answer", hir::HirIntegerConstant::Signed32(42)),
            ("narrow", hir::HirIntegerConstant::Signed8(130)),
            ("unsigned", hir::HirIntegerConstant::Unsigned8(4)),
            ("unsignedNegated", hir::HirIntegerConstant::Unsigned8(252)),
            ("nested", hir::HirIntegerConstant::Signed16(11)),
            (
                "quotient",
                hir::HirIntegerConstant::Signed32((-5i32) as u32),
            ),
            (
                "remainder",
                hir::HirIntegerConstant::Signed32((-2i32) as u32),
            ),
            ("negated", hir::HirIntegerConstant::Signed8(126)),
            ("positive", hir::HirIntegerConstant::Signed8(126)),
        ] {
            assert_eq!(
                value(name),
                &hir::ConstPropertyValue::Integer(expected),
                "{name}"
            );
        }
        for name in ["order", "equal"] {
            assert_eq!(value(name), &hir::ConstPropertyValue::Boolean(true));
        }
        let (_, global) = export
            .globals
            .iter()
            .find(|(_, global)| export.properties[global.property].name == "staticValue")
            .unwrap();
        assert!(matches!(
            global.storage,
            hir::GlobalStorage::Managed {
                state: hir::HirStaticInitialState::EncodedStaticValue {
                    payload: hir::HirConstantImage::Integer(hir::HirIntegerConstant::Signed8(11)),
                }
            }
        ));
        assert!(
            output
                .imported_core()
                .callable_selections()
                .next()
                .is_none()
        );
        assert!(
            export
                .functions
                .iter()
                .all(|(_, function)| !matches!(function.kind, hir::FunctionKind::Intrinsic(_)))
        );
    });
}

#[test]
fn imported_core_const_errors_keep_precise_operator_diagnostics() {
    for (source, expression, message) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-imported-core-const/division-zero.scoop"
            )),
            "12 / (3 - 3",
            "division by zero in const initializer",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-imported-core-const/type-mismatch.scoop"
            )),
            "1 + 2",
            "const initializer of `broken` must be of type Boolean, found Int",
        ),
    ] {
        with_input(source, |input| {
            let diagnostics = lower_ordinary(scoop_identity::RequestedConeKind::Library, input)
                .err()
                .expect("invalid const must be diagnosed");
            let start = source.find(expression).unwrap() as u32;
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message == message
                        && diagnostic.file == 0
                        && diagnostic.span
                            == Some(ast::Span {
                                start,
                                end: start + expression.len() as u32
                            })),
                "{diagnostics:?}"
            );
        });
    }
}
