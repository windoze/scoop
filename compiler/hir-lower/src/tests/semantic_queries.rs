//! Fixed queries consume ordinary source and dependency outputs.

use scoop_hir as hir;
use scoop_identity::{CallableTemplateOrigin, ConeCoordinate};

use super::m23_ordinary_core_only::support::trusted_core;
use super::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use crate::semantic_queries::FrontendQueries;
use hir::{SemanticBodyAvailability as Availability, SemanticQueries};

mod support;
mod types;

fn body_reference(declaration: CallableTemplateOrigin) -> hir::SemanticCallable {
    use hir::DefaultCallableDeclarationV1 as Body;
    hir::SemanticCallable::Imported(match declaration {
        CallableTemplateOrigin::Function(id) => Body::Function(id),
        CallableTemplateOrigin::GenericFunction(id) => Body::GenericFunction(id),
        _ => panic!("source function in this test"),
    })
}

#[test]
fn shared_queries_distinguish_published_missing_and_bodyless_dependency_functions() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "query-provider", "1.0.0").unwrap();
    let (foundation, interface) = project_dependency_text(
        &core,
        &coordinate,
        "public annotation class Note(val text: String)\n\
         @Note(\"provider\") public fun <T> identity(value: T): T = value\n\
         public fun opaque(): Int = 7\n\
         public interface Reader<T> { public fun read(): T }",
        &[],
    );
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 183);
    let world = hir::ImportedSemanticWorld::from_dependencies(
        scoop_identity::ConeIdentity::SINGLE_FILE,
        vec![
            core.provider(),
            hir::ImportedProviderInput {
                foundation: &imported,
                interface: &interface,
                alias_expansions: &aliases,
            },
        ],
        vec![],
    )
    .unwrap();
    let selection = world.dependency_selection_plan().unwrap();
    let mut checked = 0;
    let mut lowerer = crate::Lowerer::new().with_imported_dependencies(selection.clone());
    let queries = FrontendQueries {
        lowerer: &mut lowerer,
    };
    for interface in interface.callable_interfaces().all_declarations() {
        let reference = interface.declaration();
        let declaration = selection.callable_declaration(reference).unwrap();
        match declaration.name() {
            "identity" => {
                let Some(Availability::Available(hir::SemanticBody::Imported(body))) =
                    queries.body(body_reference(reference))
                else {
                    panic!("the generic function publishes its typed body")
                };
                assert_eq!(body.source_name().unwrap().as_str(), "identity");
                checked += 1;
            }
            "opaque" => {
                assert!(matches!(
                    queries.body(body_reference(reference)),
                    Some(Availability::NotPublished)
                ));
                checked += 1;
            }
            "read" => {
                assert!(matches!(
                    queries.body(body_reference(reference)),
                    Some(Availability::NoBody)
                ));
                checked += 1;
            }
            _ => {}
        }
    }
    assert_eq!(checked, 3);
    let target = interface
        .annotations()
        .targets()
        .iter()
        .find(|target| matches!(target.target, hir::AnnotationTargetV1::Callable(_)))
        .unwrap();
    let hir::SemanticAnnotations::Imported(applications) =
        queries.describe_annotations(hir::SemanticAnnotationTarget::Imported(target.target))
    else {
        panic!("dependency annotations")
    };
    assert_eq!(applications.len(), 1);
    assert!(matches!(
        queries.annotation_declaration(applications[0].annotation),
        Some(hir::SemanticAnnotationDeclaration::Imported(_))
    ));
    let no_dependencies = crate::Lowerer::new().with_imported_dependencies(
        hir::ImportedDependencySelectionPlan::empty(scoop_identity::ConeIdentity::SINGLE_FILE),
    );
    let mut no_dependencies = no_dependencies;
    let missing = FrontendQueries {
        lowerer: &mut no_dependencies,
    };
    let reference = interface
        .callable_interfaces()
        .all_declarations()
        .next()
        .unwrap()
        .declaration();
    assert!(missing.body(body_reference(reference)).is_none());
}

#[test]
fn inherited_default_query_preserves_the_dependency_definition_scope() {
    struct InspectDefault(bool);
    impl hir::MethodBodyGenerator for InspectDefault {
        fn generate(
            &mut self,
            queries: &mut dyn SemanticQueries,
        ) -> Result<Vec<hir::GeneratedMethodBody>, Vec<scoop_ast::Diagnostic>> {
            let function = queries
                .annotations()
                .targets
                .iter()
                .find_map(|target| match target.target {
                    hir::SourceAnnotationTarget::Callable(
                        hir::SourceAnnotationCallable::Function(id),
                    ) => Some(id),
                    _ => None,
                })
                .unwrap();
            let signature = queries.signature(function);
            let hir::ExportParameterCalling::Default { source, value_type } =
                signature.parameters[1].calling
            else {
                panic!("ordinary inherited default")
            };
            let default = queries.default_source(source);
            let hir::SemanticDefaultDefinition::Imported(template) = default.definition else {
                panic!("the inherited definition remains a dependency template")
            };
            assert_eq!(default.type_arguments, &[value_type]);
            assert_ne!(
                template.definition_origin().origin().source().cone(),
                queries.current_cone()
            );
            assert_eq!(template.value_parameters().parameters().len(), 1);
            self.0 = true;
            Ok(Vec::new())
        }
    }
    let mut inspection = InspectDefault(false);
    support::with_provider(
        "public annotation class Mark\npublic interface Picker<T> { public fun choose(first: T, second: T = first): T }",
        "class Choice<T> : Picker<T> { @Mark public override fun choose(first: T, second: T): T = second }",
        &mut inspection,
    );
    assert!(inspection.0);
}
