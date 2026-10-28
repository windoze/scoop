use super::*;
use crate::tests::m23_ordinary_core_only::support::parsed_ordinary_text;
use scoop_hir as hir;

#[test]
fn aggregate_const_values_survive_dependency_materialization() {
    use crate::tests::m23_ordinary_dependencies::support::project_dependency_text;
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "aggregate-constant-provider", "1.0.0").unwrap();
    let (foundation, interface) = project_dependency_text(
        &core,
        &coordinate,
        r#"
        package dependency.api
        public class RuntimePayload(val number: Int)
        public enum Choice<T> { Empty, Value(T) }
        public const val aggregate: (Unit, (String, String), Choice<RuntimePayload>) =
            ((), ("first", "second"), Choice.Empty)
        public fun<T> labels(value: T, text: (String, String) = aggregate._2): (String, String) = text
    "#,
        &["Unit", "String", "Int"],
    );
    let foundation = core.import_dependency_foundation(&coordinate, &foundation, 57);
    let fixture = DependencyConstantFixture {
        core,
        coordinate,
        foundation,
        interface,
        aliases: empty_alias_expansions(),
    };
    let consumer = scoop_parser::parse(
        r#"
        import dependency.api.*
        public const val copied: (Unit, (String, String), Choice<RuntimePayload>) = aggregate
        public const val selected: String = copied._2._2
        fun read(): (Unit, (String, String), Choice<RuntimePayload>) = aggregate
        fun defaultLabels(): (String, String) = labels(1)
    "#,
    )
    .unwrap();
    fixture.inspect(consumer, |output| {
        let module = output.output().export.module();
        assert!(matches!(function_result(module, "read").kind, hir::ExprKind::TupleLiteral(_)));
        let selected = module.properties.iter().find(|(_, property)| property.name == "selected").unwrap().1;
        assert!(matches!(&selected.representation, hir::PropertyRepresentation::Const { value: hir::HirConstantImage::String(value) } if value == "second"));
        let values = hir::CanonicalExportConstValuesV1::from_export_hir(module).unwrap();
        let copied = module.properties.iter().find(|(_, property)| property.name == "copied").unwrap().0;
        let identity = module.property_identities[copied].ordinary_id().unwrap();
        let hir::CanonicalValueConstantV1::Tuple(elements) = values.get(identity).unwrap().value() else { panic!("copied constant remains a tuple") };
        assert!(matches!(&elements[2], hir::CanonicalValueConstantV1::Enum { fields, .. } if fields.is_empty()));
    });
}

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
