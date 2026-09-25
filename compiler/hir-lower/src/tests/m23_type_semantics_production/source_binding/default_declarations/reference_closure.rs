use super::*;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/reference-closure.scoop"
));

#[test]
fn reference_closure_is_part_of_the_atomic_source_declaration_transaction() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols)
                .unwrap();
            let bound = parameters.bind_default_declarations(&table, &[]).unwrap();
            let contract = &bound.declarations()[0];
            let template = table.get(contract.key()).unwrap();
            assert_eq!(contract.references().template(), contract.key());
            let refs = template.references();
            let total = refs.callables().len()
                + refs.constructors().len()
                + refs.types().len()
                + refs.globals().len()
                + refs.singleton_values().len()
                + refs.fields().len();
            assert_eq!(contract.references().occurrences().len(), total);
            let incomplete = without_last_callable(template);
            let changed = replace(&table, incomplete);
            let Error::Record { key, error } = parameters
                .bind_default_declarations(&changed, &[])
                .unwrap_err()
            else {
                panic!("expected record error")
            };
            assert_eq!(key, contract.key());
            assert!(matches!(*error, Error::ReferenceClosure(error)
                if matches!(*error, hir::DefaultSourceReferenceClosureError::Missing {
                    kind: hir::ExportDefaultReferenceKindV1::Callable, index: 2, ..
                })
            ));
        });
    });
}
fn without_last_callable(t: &Template) -> Template {
    let refs = t.references();
    let mut callables = refs.callables().to_vec();
    callables.pop();
    let refs = hir::DefaultSourceReferencesV1::try_new(
        callables,
        refs.constructors().to_vec(),
        refs.types().to_vec(),
        refs.globals().to_vec(),
        refs.singleton_values().to_vec(),
        refs.fields().to_vec(),
    )
    .unwrap();
    Template::try_new(
        t.key(),
        t.definition_root(),
        t.definition_path().clone(),
        t.locals().clone(),
        t.body().clone(),
        t.result().clone(),
        t.allows_suspend(),
        t.type_parameters().clone(),
        t.receiver().clone(),
        t.value_parameters().clone(),
        refs,
        t.definition_origin().clone(),
    )
    .unwrap()
}
