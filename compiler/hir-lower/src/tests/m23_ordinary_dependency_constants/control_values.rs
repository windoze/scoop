use super::*;
use crate::tests::m23_ordinary_core_only::support::parsed_ordinary_text;
use scoop_hir as hir;

#[test]
fn branch_temporaries_use_evaluation_spans_and_preserve_imported_constants() {
    let fixture = DependencyConstantFixture::new();
    for expression in [
        "if (flag) answer else answer",
        "when (flag) { case true -> answer; case false -> answer }",
        "try { answer } finally { answer }",
    ] {
        let source =
            format!("import dependency.api.answer\nfun probe(flag: Boolean): Int = {expression}");
        let ordinary = parsed_ordinary_text(&source);
        let world = hir::ImportedSemanticWorld::from_dependencies(
            ordinary.cone(),
            vec![
                fixture.core.provider(),
                hir::ImportedProviderInput {
                    foundation: &fixture.foundation,
                    interface: &fixture.interface,
                    alias_expansions: &fixture.aliases,
                },
            ],
            Vec::new(),
        )
        .unwrap();
        let core = fixture
            .core
            .foundation
            .import_core_inputs(&fixture.core.interface)
            .unwrap();
        let input = CurrentConeSources::try_new(&ordinary, core, &world).unwrap();
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
            .expect("control values may use dependency constants");
        let module = output.output().export.module();
        let function = module
            .functions
            .iter()
            .find_map(|(_, function)| (function.name == "probe").then_some(function))
            .unwrap();
        let hir::FunctionKind::User(body) = &function.kind else {
            panic!("probe has an ordinary body");
        };
        let mut locations = Vec::new();
        collect_constant_statements(&body.statements, &mut locations);
        assert_eq!(locations.len(), 2, "{expression}");
        for (span, value) in locations {
            let origin = value.origin.concrete();
            assert_eq!(span, origin.evaluation.span, "{expression}");
            assert_eq!(&source[span.start as usize..span.end as usize], "answer");
            assert_eq!(
                source_cone(module, origin.definition.file),
                fixture.coordinate.identity().unwrap()
            );
            assert_eq!(source_cone(module, origin.evaluation.file), module.cone);
        }
        hir::CanonicalHirFoundation::from_dependency_output(&output)
            .expect("control temporaries retain valid source points");
    }
}

fn collect_constant_statements<'a>(
    statements: &'a [hir::Statement],
    locations: &mut Vec<(scoop_ast::Span, &'a hir::Expr)>,
) {
    for statement in statements {
        match &statement.kind {
            hir::StatementKind::Assign { value, .. } | hir::StatementKind::Expr(value)
                if matches!(
                    value.kind,
                    hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(42))
                ) =>
            {
                locations.push((statement.span, value));
            }
            hir::StatementKind::If {
                then_body,
                else_body,
                ..
            } => {
                collect_constant_statements(then_body, locations);
                if let Some(body) = else_body {
                    collect_constant_statements(body, locations);
                }
            }
            hir::StatementKind::When(value) => {
                for arm in value.arms() {
                    collect_constant_statements(arm.body, locations);
                }
            }
            hir::StatementKind::Try(value) => {
                collect_constant_statements(&value.body, locations);
                if let Some(body) = &value.finally_body {
                    collect_constant_statements(body, locations);
                }
            }
            _ => {}
        }
    }
}
