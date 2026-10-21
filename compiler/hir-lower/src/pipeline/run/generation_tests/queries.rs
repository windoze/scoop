use super::*;
use hir::{SemanticBody as Body, SemanticBodyAvailability as Availability, SemanticCallable};

fn marked_functions(queries: &dyn SemanticQueries) -> Vec<hir::FunctionId> {
    queries
        .annotations()
        .targets
        .iter()
        .filter_map(|entry| match entry.target {
            hir::SourceAnnotationTarget::Callable(hir::SourceAnnotationCallable::Function(id)) => {
                Some(id)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn queries_preserve_empty_bodies_abstract_slots_and_pending_generation() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let functions = marked_functions(queries);
        assert_eq!(functions.len(), 3);
        for function in functions {
            let signature = queries.signature(function);
            let body = queries.body(SemanticCallable::Current(function)).unwrap();
            match signature.name.as_str() {
                "empty" => {
                    let Availability::Available(Body::Current(body)) = body else {
                        panic!("an empty ordinary body is available")
                    };
                    assert!(body.statements.is_empty());
                }
                "result" => {
                    let Availability::Available(Body::Current(body)) = body else {
                        panic!("ordinary typed body")
                    };
                    assert!(matches!(
                        body.statements[0].kind,
                        hir::StatementKind::Return { .. }
                    ));
                }
                "read" => assert!(matches!(body, Availability::NoBody)),
                name => panic!("unexpected marked function: {name}"),
            }
        }
        let method = queries.pending_methods()[0].function;
        assert!(matches!(
            queries.body(SemanticCallable::Current(method)),
            Some(Availability::PendingGeneration)
        ));
        Ok(vec![generated(queries, method, "{ return 1 }")])
    });
    compile(
        "annotation class Mark\n\
         @Mark fun empty() {}\n\
         @Mark fun result(): Int = 1\n\
         interface I { @Mark fun read(): Int }\n\
         class C : I",
        &mut driver,
    )
    .unwrap();
}

#[test]
fn default_query_keeps_definition_binders_and_previous_parameter_binding() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let signature = queries.signature(function);
        let hir::ExportParameterCalling::Default { source, value_type } =
            signature.parameters[1].calling
        else {
            panic!("ordinary default parameter")
        };
        let default = queries.default_source(source);
        let hir::SemanticDefaultDefinition::Current { template, .. } = default.definition else {
            panic!("current source template")
        };
        assert_eq!(default.type_arguments, &[value_type]);
        assert_eq!(template.type_parameters, [signature.type_parameters[0].id]);
        assert_eq!(template.value_parameters.len(), 1);
        assert_eq!(template.value_parameters[0].position, 0);
        assert!(matches!(template.value.kind, hir::ExprKind::Local(local)
            if local == template.value_parameters[0].local));
        assert_eq!(template.value.ty, value_type);
        assert!(
            matches!(template.definition_root, hir::LexicalDefinitionRoot::Function(id)
            if id == function)
        );
        assert_eq!(template.origin.file, signature.parameters[1].origin.file);
        let location = queries.source_location(template.origin);
        assert_eq!(location.source, signature.origin.source);
        assert_eq!(location.span, template.origin.span);
        Ok(Vec::new())
    });
    compile(
        "annotation class Mark\n\
         @Mark fun <T> choose(first: T, second: T = first): T = second",
        &mut driver,
    )
    .unwrap();
}

#[test]
fn annotation_queries_keep_actual_identity_and_each_array_elements_source() {
    let source = "annotation class Labels(val values: Array<String> = [\"default\"])\n\
                  @Labels([\"读\", \"write\"]) fun marked() {}";
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let target = hir::SourceAnnotationTarget::Callable(
            hir::SourceAnnotationCallable::Function(function),
        );
        let hir::SemanticAnnotations::Current(annotations) =
            queries.describe_annotations(hir::SemanticAnnotationTarget::Current(target))
        else {
            panic!("source annotations")
        };
        assert_eq!(annotations.len(), 1);
        let application = &annotations[0];
        let Some(hir::SemanticAnnotationDeclaration::Current(declaration)) =
            queries.annotation_declaration(application.annotation)
        else {
            panic!("actual declaration")
        };
        assert_eq!(declaration.identity.id(), application.annotation);
        assert!(declaration.parameters[0].default.is_some());
        let hir::CanonicalAnnotationValueV1::Array { elements, .. } = &application.arguments[0]
        else {
            panic!("ordinary normalized annotation array")
        };
        assert_eq!(elements.len(), 2);
        for (element, text) in elements.iter().zip(["\"读\"", "\"write\""]) {
            assert_eq!(
                element.source,
                crate::tests::test_source_identity("src/user.scoop")
            );
            assert_eq!(
                &source[element.span.start_byte() as usize..element.span.end_byte() as usize],
                text
            );
        }
        Ok(Vec::new())
    });
    compile(source, &mut driver).unwrap();
}
