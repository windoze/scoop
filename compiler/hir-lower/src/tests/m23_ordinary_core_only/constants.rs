use super::support::{TrustedCoreFixture, trusted_core_from_source};
use super::*;
use crate::tests::complete_core_file;
use scoop_ast as ast;
use scoop_hir as hir;

mod aggregates;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/language/imported-core-const/operators.scoop"
));

pub(super) fn with_input<R>(source: &str, run: impl FnOnce(&CurrentConeSources<'_, '_>) -> R) -> R {
    with_core_input(source, trusted_core(), run)
}

fn with_core_input<R>(
    source: &str,
    core: TrustedCoreFixture,
    run: impl FnOnce(&CurrentConeSources<'_, '_>) -> R,
) -> R {
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
    let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(sources.cone());
    let input = CurrentConeSources::try_new(&sources, protocols, &world).unwrap();
    run(&input)
}

#[test]
fn lambda_tail_statements_use_imported_constants_evaluation_spans() {
    let constant = "public const val LAMBDA_CONSTANT: Int = 42\n";
    let mut core = complete_core_file();
    core.declarations
        .extend(scoop_parser::parse(constant).unwrap().declarations);
    let core = trusted_core_from_source(core, constant);
    let source = "fun returnsConstant(): () -> Int = { LAMBDA_CONSTANT }\n\
                  fun discardsConstant(): () -> Unit = { LAMBDA_CONSTANT }\n";
    with_core_input(source, core, |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input).unwrap();
        let export = output.output().export.module();
        assert_eq!(export.lambdas.len(), 2);
        let mut returned = 0;
        let mut discarded = 0;
        for (_, lambda) in export.lambdas.iter() {
            let function = &export.functions[lambda.definition.source_function()];
            let hir::FunctionKind::User(body) = &function.kind else {
                panic!("lambda body");
            };
            for statement in &body.statements {
                let value = match &statement.kind {
                    hir::StatementKind::Return { value }
                        if !matches!(value.kind, hir::ExprKind::UnitLiteral) =>
                    {
                        returned += 1;
                        value
                    }
                    hir::StatementKind::Expr(value) => {
                        discarded += 1;
                        value
                    }
                    _ => continue,
                };
                let origin = value.origin.concrete();
                assert_ne!(origin.definition.provider, origin.evaluation.provider);
                assert_eq!(statement.span, origin.evaluation.span);
                assert_eq!(
                    &source[statement.span.start as usize..statement.span.end as usize],
                    "LAMBDA_CONSTANT"
                );
            }
        }
        assert_eq!((returned, discarded), (1, 1));
    });
}

#[test]
fn imported_core_integer_protocols_fold_const_and_static_operators() {
    with_input(SOURCE, |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input).unwrap();
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
            export
                .functions
                .iter()
                .all(|(_, function)| !matches!(function.kind, hir::FunctionKind::Intrinsic(_)))
        );
    });
}
