use super::*;
use crate::tests::m23_ordinary_core_only::support::parsed_ordinary_text;
use scoop_hir as hir;

#[test]
fn aggregate_temporaries_use_consumer_spans_and_keep_constant_origins() {
    let fixture = DependencyConstantFixture::new();
    for aggregate in ["[answer, answer + 1]", "(answer, answer + 1)"] {
        let source =
            format!("import dependency.api.answer\nfun probe() {{ val values = {aggregate} }}");
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
            .expect("an imported constant may precede an element with statement expansion");
        let module = output.output().export.module();
        let function = module
            .functions
            .iter()
            .find_map(|(_, function)| (function.name == "probe").then_some(function))
            .unwrap();
        let hir::FunctionKind::User(body) = &function.kind else {
            panic!("probe has an ordinary body");
        };
        let (statement, initializer) = body
            .statements
            .iter()
            .find_map(|statement| match &statement.kind {
                hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Binding { local },
                    init,
                } if body.locals[*local].name == "$aggregate.0" => Some((statement, init)),
                _ => None,
            })
            .expect("the first element is materialized before the following addition");
        let start = u32::try_from(source.find(aggregate).unwrap() + 1).unwrap();
        let expected = scoop_ast::Span {
            start,
            end: start + 6,
        };
        assert_eq!(statement.span, expected);
        let origin = initializer.origin.concrete();
        assert_eq!(origin.evaluation.span, expected);
        assert_eq!(
            source_cone(module, origin.definition.file),
            fixture.coordinate.identity().unwrap()
        );
        assert_eq!(source_cone(module, origin.evaluation.file), module.cone);
        hir::CanonicalHirFoundation::from_dependency_output(&output)
            .expect("aggregate temporaries must project locations in the consumer source");
    }
}
