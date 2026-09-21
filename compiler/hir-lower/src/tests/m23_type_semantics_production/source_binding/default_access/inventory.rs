use super::*;

#[test]
fn default_access_binding_derives_demand_owners_from_actual_keys() {
    with_sources(SOURCE, |output, fixture, _, _| {
        let subject = function(output.output().export.module(), "AccessRoot.Hidden.leaf");
        let required = BTreeSet::from([subject]);
        let table =
            Table::from_export_hir(&output.output().export, &required, &mut meter()).unwrap();
        assert_eq!(table.records().len(), 3);
        let foundation = fixture.bind().unwrap();
        let bound = foundation
            .bind_default_access_declarations(&table, &required, &mut meter())
            .unwrap();
        let original = bound.declaration(subject, &mut meter()).unwrap();
        let access = original.declaration_access();
        let root = nominal_subject(access.lexical_owners()[0]);
        let changed = access_record(
            original,
            access.declared_visibility(),
            access.lexical_owners()[1..].to_vec(),
            access.definition_origin().clone(),
        );
        let changed = replacing(&table, changed);
        let records = changed
            .records()
            .iter()
            .filter(|r| r.subject() != root)
            .cloned()
            .collect();
        let changed = Table::try_new(records, &mut meter()).unwrap();
        assert!(
            matches!(foundation.bind_default_access_declarations(&changed, &required, &mut meter()),
            Err(Error::MissingRecord(id)) if id == root)
        );
    });
}
#[test]
fn default_access_binding_requires_exact_demand_coverage_and_closed_subject_roles() {
    with_sources(OUTSIDE_ROOTS, |output, fixture, required, table| {
        let foundation = fixture.bind().unwrap();
        let first = table.records()[0].subject();
        let missing = Table::try_new(table.records()[1..].to_vec(), &mut meter()).unwrap();
        assert!(
            matches!(foundation.bind_default_access_declarations(&missing, required, &mut meter()),
            Err(Error::MissingRecord(id)) if id == first)
        );
        assert!(
            matches!(foundation.bind_default_access_declarations(table, &BTreeSet::new(), &mut meter()),
            Err(Error::UnexpectedRecord(id)) if id == first)
        );
        let field = output
            .output()
            .export
            .module()
            .export_definition_origins
            .records()
            .iter()
            .find(|r| matches!(r.subject(), Subject::Field(_)))
            .unwrap()
            .subject();
        assert!(
            matches!(foundation.bind_default_access_declarations(table, &BTreeSet::from([field]), &mut meter()),
            Err(Error::InvalidSubject(id)) if id == field)
        );
    });
}
