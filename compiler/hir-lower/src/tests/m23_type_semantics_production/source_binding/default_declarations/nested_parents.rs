use super::*;
use hir::{
    DefaultSourceNestedCallableDescriptorV1 as Descriptor,
    DefaultSourceNestedIdentityFailureV1 as Failure,
};
use scoop_wire::WirePath;

pub(super) const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/nested-parents.scoop"
));

fn first(template: &Template) -> hir::DefaultSourceNestedCallableOccurrenceV1<'_> {
    template
        .index_nested_callables(&mut meter(), &WirePath::root())
        .unwrap()
        .occurrences()[0]
}

fn transplanted(original: &Template, other: &Template) -> Template {
    let occurrence = first(original);
    let from = occurrence.descriptor();
    let to = first(other).descriptor();
    assert_eq!(from.identity().kind(), to.identity().kind());
    assert_ne!(from.identity(), to.identity());
    assert_eq!(from.definition_path(), to.definition_path());
    assert_eq!(from.function_type(), to.function_type());
    assert_eq!(from.captures(), to.captures());
    let body = match to {
        Descriptor::LocalFunction(function) => {
            let statement = hir::DefaultStatementV1::try_new(
                hir::DefaultStatementKindV1::LocalFunction(function.clone()),
                occurrence.definition_origin().clone(),
            )
            .unwrap();
            let mut statements = vec![statement];
            statements.extend_from_slice(original.body().statements());
            hir::ExportDefaultBodyV1::try_new(statements, original.body().value().clone()).unwrap()
        }
        _ => hir::ExportDefaultBodyV1::try_new(
            original.body().statements().to_vec(),
            hir::DefaultExpressionV1::try_new(
                other.body().value().kind().clone(),
                original.body().value().result_type().clone(),
                original.body().value().definition_origin().clone(),
            )
            .unwrap(),
        )
        .unwrap(),
    };
    envelope::rebuild(original, original.locals().records().to_vec(), body)
}

#[test]
fn default_source_nested_parent_rejects_same_path_from_another_method_or_variant() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let mut changed = ["lambda", "anonymous", "reference", "local"]
            .map(|name| {
                let original = table
                    .get(key(output, &format!("NestedParentHost.{name}A"), 0))
                    .unwrap();
                let other = table
                    .get(key(output, &format!("NestedParentHost.{name}B"), 0))
                    .unwrap();
                transplanted(original, other)
            })
            .to_vec();
        let variants: Vec<_> = table
            .records()
            .iter()
            .filter(|t| {
                matches!(
                    t.key().owner(),
                    CallableTemplateOrigin::VariantConstructor(_)
                )
            })
            .collect();
        assert_eq!(variants.len(), 2);
        changed.push(transplanted(variants[0], variants[1]));
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            parameters
                .bind_default_declarations(&table, &[], &mut meter())
                .unwrap();
            for template in changed {
                let expected = template.key();
                let Error::Record { key, error } = parameters
                    .bind_default_declarations(&replace(&table, template), &[], &mut meter())
                    .unwrap_err()
                else {
                    panic!("expected nested parent error");
                };
                assert_eq!(key, expected);
                assert!(
                    matches!(
                        *error,
                        Error::NestedIdentity {
                            reason: Failure::DefinitionContext,
                            ..
                        }
                    ),
                    "{error:?}"
                );
            }
        });
    });
}

#[test]
fn default_source_expanded_local_declarations_retain_the_provider_context() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let provider = source_template(output, "nestedParentProvider", 0);
        let table = templates(output);
        let expanded = table
            .get(key(output, "NestedParentHost.expanded", 0))
            .unwrap();
        let original = first(&provider);
        let copy = first(expanded);
        assert!(matches!(
            original.descriptor(),
            Descriptor::LocalFunction(_)
        ));
        assert_eq!(
            original.descriptor().identity(),
            copy.descriptor().identity()
        );
        assert_eq!(original.definition_origin(), copy.definition_origin());
        assert_ne!(
            copy.definition_origin().origin().context(),
            expanded.definition_origin().origin().context()
        );
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            parameters
                .bind_default_declarations(&table, &[], &mut meter())
                .unwrap();
        });
    });
}
