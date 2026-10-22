//! Fixed queries consume ordinary source and dependency outputs.

use scoop_hir as hir;
use scoop_identity::{CallableTemplateOrigin, ConeCoordinate};

use super::m23_ordinary_core_only::support::trusted_core;
use super::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use crate::semantic_queries::FrontendQueries;
use hir::{SemanticBodyAvailability as Availability, SemanticQueries};

mod annotations;
mod constructors;
mod declaration_references;
mod exports;
mod fields;
mod methods;
mod projected_bodies;
mod properties;
mod signatures;
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
    let original = lowerer.clone();
    let mut queries = FrontendQueries::new(&mut lowerer, &original, Some(&world));
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
                let Some(Availability::Available(projected)) =
                    queries.projected_body(body_reference(reference)).unwrap()
                else {
                    panic!("shared dependency body")
                };
                assert!(std::sync::Arc::ptr_eq(&projected, &body.shared_body()));
                let json = serde_json::to_value(scoop_generator_protocol::BodyJson::new(
                    projected.as_ref(),
                ))
                .unwrap();
                assert_eq!(
                    json["owner"],
                    serde_json::json!(scoop_generator_protocol::CallableDeclaration::from(
                        reference
                    ))
                );
                assert_eq!(
                    json["origin"]["source"]["cone"],
                    coordinate.identity().unwrap().to_string()
                );
                assert_eq!(
                    json["result"],
                    serde_json::json!({"kind": "binder", "depth": 0, "index": 0})
                );
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
    let json = serde_json::to_value(scoop_generator_protocol::Annotations {
        queries: &queries,
        target: hir::SemanticAnnotationTarget::Imported(target.target),
    })
    .unwrap();
    assert_eq!(
        json[0]["annotation"],
        applications[0].annotation.to_string()
    );
    assert_eq!(json[0]["arguments"][0]["value"]["value"], "provider");
    assert_eq!(
        json[0]["origin"]["source"]["cone"],
        coordinate.identity().unwrap().to_string()
    );
    assert!(matches!(
        queries.annotation_declaration(applications[0].annotation),
        Some(hir::SemanticAnnotationDeclaration::Imported(_))
    ));
    let no_dependencies = crate::Lowerer::new().with_imported_dependencies(
        hir::ImportedDependencySelectionPlan::empty(scoop_identity::ConeIdentity::SINGLE_FILE),
    );
    let mut no_dependencies = no_dependencies;
    let original = no_dependencies.clone();
    let missing = FrontendQueries::new(&mut no_dependencies, &original, None);
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
            let definition = template.definition_origin().clone();
            let root = template.definition_root();
            let projected = queries
                .projected_default(hir::SemanticDefaultReference::Current(source))
                .unwrap()
                .unwrap();
            assert_eq!(projected.definition_origin(), &definition);
            assert_eq!(projected.definition_root(), root);
            assert_ne!(projected.key().owner(), root.declaration());
            let binders = signature.declaration_binders();
            let application = queries
                .default_application(
                    (&binders).into(),
                    &hir::SemanticDefaultUse {
                        definition: hir::SemanticDefaultReference::Current(source),
                        declaration_arguments: vec![value_type],
                        arguments: vec![value_type],
                    },
                )
                .unwrap()
                .unwrap();
            assert_eq!(application.provider().nominal_owner_binder_arity(), 1);
            assert_eq!(application.provider().callable_own_binder_arity(), 0);
            assert_eq!(
                application.applied_type(projected.result()).unwrap(),
                queries
                    .type_signature((&binders).into(), value_type)
                    .unwrap(),
            );
            let json =
                serde_json::to_value(scoop_generator_protocol::applied_default(&application))
                    .unwrap();
            assert_ne!(json["key"]["owner"], json["definitionRoot"]);
            assert_eq!(
                json["origin"]["source"]["cone"],
                projected
                    .definition_origin()
                    .origin()
                    .source()
                    .cone()
                    .to_string()
            );
            assert_eq!(json["previousParameters"].as_array().unwrap().len(), 1);
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
