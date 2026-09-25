use super::*;

fn rejected(
    fixture: &Fixture,
    foundation: &hir::OdrFreeHirFoundation,
    table: &Table,
    required: &BTreeSet<Subject>,
) -> Error {
    match fixture
        .source
        .bind_to_foundation(foundation, &fixture.identities)
    {
        Ok(bound) => bound
            .bind_default_access_declarations(table, required)
            .unwrap_err(),
        Err(error) => error.into(),
    }
}

#[test]
fn default_access_binding_rejects_graph_only_keys_in_each_typed_artifact_table() {
    with_sources(OUTSIDE_ROOTS, |_, fixture, required, table| {
        assert!(fixture.source.entries().sources.records().is_empty());
        for tag in 1..=8 {
            let mut canonical = fixture.foundation.as_canonical().clone();
            match tag {
                1 => canonical.set_types(vec![]).unwrap(),
                2 => canonical.set_generic_types(vec![]).unwrap(),
                3 => canonical.set_functions(vec![]).unwrap(),
                4 => canonical.set_generic_functions(vec![]).unwrap(),
                5 => canonical.set_constructors(vec![]).unwrap(),
                6 => canonical.set_properties(vec![]).unwrap(),
                7 => canonical.set_extension_properties(vec![]).unwrap(),
                8 => canonical.set_property_accessors(vec![]).unwrap(),
                _ => unreachable!(),
            }
            let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let error = rejected(fixture, &incomplete, table, required);
            // The foundation prerequisite already binds every accessor key.
            assert!(
                match error {
                    Error::MissingKey(id) => id.kind_tag() == tag,
                    Error::Foundation(error) if tag == 8 =>
                        matches!(*error, hir::TypeFoundationBindingError::MissingAccessor(_)),
                    _ => false,
                },
                "tag={tag}"
            );
        }
    });
}
#[test]
fn default_access_binding_requires_identity_graph_and_local_provider() {
    with_sources(OUTSIDE_ROOTS, |_, fixture, _, table| {
        let empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        assert!(matches!(
            fixture
                .source
                .bind_to_foundation(&fixture.foundation, &empty),
            Err(hir::TypeFoundationBindingError::Identity(_))
        ));
        let foreign = scoop_identity::CoreBuiltinNominal::Any.identity_record();
        let subject = Subject::Type(foreign.id());
        let mut canonical = fixture.foundation.as_canonical().clone();
        canonical.set_types(vec![foreign]).unwrap();
        let canonical = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let foundation = fixture
            .source
            .bind_to_foundation(&canonical, &fixture.identities)
            .unwrap();
        let table = Table::try_new(vec![
            Record::try_new(subject, table.records()[0].declaration_access().clone()).unwrap(),
        ])
        .unwrap();
        assert!(
            matches!(foundation.bind_default_access_declarations(&table, &BTreeSet::from([subject])),
            Err(Error::ForeignKey(id)) if id == subject)
        );
    });
}
#[test]
fn default_access_origins_require_actual_subject_context_file_and_span_points() {
    with_sources(OUTSIDE_ROOTS, |output, fixture, _, table| {
        let subject = function(output.output().export.module(), "privateTarget");
        let required = BTreeSet::from([subject]);
        let record = table.get(subject).unwrap();
        let table = Table::try_new(vec![record.clone()]).unwrap();
        for field in [1, 2, 3] {
            let mut canonical = fixture.foundation.as_canonical().clone();
            match field {
                1 => canonical.set_source_contexts(vec![]).unwrap(),
                2 => canonical.set_sources(vec![]).unwrap(),
                3 => canonical.set_definition_origins(vec![]).unwrap(),
                _ => unreachable!(),
            }
            let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let error = rejected(fixture, &incomplete, &table, &required);
            assert!(match (field, error) {
                (1, Error::Foundation(e)) =>
                    matches!(*e, hir::TypeFoundationBindingError::MissingSourceContext(_)),
                (2, Error::Foundation(e)) =>
                    matches!(*e, hir::TypeFoundationBindingError::MissingSourceRecord),
                (3, Error::Origin(id)) => id == subject,
                _ => false,
            });
        }
        let access = record.declaration_access();
        let origin = access.definition_origin().origin();
        let context = fixture
            .foundation
            .source_context_key(origin.context())
            .unwrap();
        let invalid = scoop_identity::DefinitionOrigin::new(
            origin.source().clone(),
            scoop_identity::SourceSpan::new(0, 1).unwrap(),
            context,
        )
        .unwrap();
        let record = access_record(
            record,
            access.declared_visibility(),
            vec![],
            hir::ExportDefinitionSourceV1::new(invalid),
        );
        let table = Table::try_new(vec![record]).unwrap();
        assert!(
            matches!(fixture.bind().unwrap().bind_default_access_declarations(&table, &required),
            Err(Error::Foundation(e)) if matches!(*e, hir::TypeFoundationBindingError::MissingSourcePoint(1)))
        );
    });
}
