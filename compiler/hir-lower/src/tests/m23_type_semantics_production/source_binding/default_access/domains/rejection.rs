use super::*;
use hir::DefaultSourceDomainReplayError as DomainError;

#[test]
fn default_source_lookup_protected_class_requires_bound_artifact_exact_evidence() {
    let source = "private class ExactHost { protected fun member(): Int = 1 }";
    with_sources(source, |output, fixture, required, table| {
        assert!(fixture.source.entries().sources.records().is_empty());
        let export = output.output().export.module();
        let subject = function(export, "ExactHost.member");
        let hir::SourceNominalId::Concrete(owner) = table
            .get(subject)
            .unwrap()
            .declaration_access()
            .lexical_owners()[0]
        else {
            panic!("concrete source class");
        };
        let expected_key = scoop_identity::ExactTypeKey::Nominal(owner);
        let exact = PersistentExactTypeId::from_key(&expected_key).unwrap();
        let foundation = fixture.bind().unwrap();
        assert_eq!(foundation.exact_type_key(exact).unwrap(), &expected_key);
        let bound = foundation
            .bind_default_access_declarations(table, required)
            .unwrap();
        assert_eq!(
            bound
                .source_lookup_domain(subject)
                .unwrap()
                .persistent()
                .constraints(),
            &[
                Constraint::Cone(export.cone),
                Constraint::File(
                    table
                        .get(subject)
                        .unwrap()
                        .declaration_access()
                        .definition_origin()
                        .origin()
                        .source()
                        .clone()
                ),
                Constraint::SubclassesOf(exact)
            ]
        );
        let mut entries = fixture.source.clone().into_entries();
        entries.exact_keys = hir::CanonicalPersistentIdsV1::try_new(
            entries
                .exact_keys
                .values()
                .iter()
                .filter(|id| **id != exact)
                .copied()
                .collect(),
        )
        .unwrap();
        let incomplete = hir::TypeFoundationSourceAuthorityV1::try_new(entries).unwrap();
        let foundation = incomplete
            .bind_to_foundation(&fixture.foundation, &fixture.identities)
            .unwrap();
        let bound = foundation
            .bind_default_access_declarations(table, required)
            .unwrap();
        assert!(matches!(bound.source_lookup_domain(subject),
            Err(DomainError::Authority(Error::Foundation(error))) if matches!(*error, hir::TypeFoundationBindingError::MissingExact(id) if id == exact)));
        let mut canonical = fixture.foundation.as_canonical().clone();
        canonical.set_exact_types(vec![]).unwrap();
        let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        assert!(matches!(
            fixture
                .source
                .bind_to_foundation(&incomplete, &fixture.identities),
            Err(hir::TypeFoundationBindingError::MissingExact(_))
        ));
    });
}

#[test]
fn default_source_lookup_queries_cannot_escape_the_bound_demand_closure() {
    with_sources(SOURCE, |output, fixture, _, _| {
        let export = output.output().export.module();
        let selected = function(export, "fileOnly");
        let other = function(export, "exposed");
        let required = BTreeSet::from([selected]);
        let table = Table::from_export_hir(&output.output().export, &required).unwrap();
        let foundation = fixture.bind().unwrap();
        let bound = foundation
            .bind_default_access_declarations(&table, &required)
            .unwrap();
        assert!(matches!(bound.source_lookup_domain(other),
            Err(DomainError::Authority(Error::MissingRecord(id))) if id == other));
    });
}
