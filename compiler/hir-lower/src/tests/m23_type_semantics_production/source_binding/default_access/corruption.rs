use super::*;

#[test]
fn default_access_binding_rejects_origin_and_owner_chain_substitution() {
    with_sources(SOURCE, |output, fixture, required, table| {
        let subject = function(output.output().export.module(), "AccessRoot.Hidden.leaf");
        let record = table.get(subject).unwrap();
        let access = record.declaration_access();
        let other = table
            .records()
            .iter()
            .find(|r| matches!(r.subject(), Subject::GenericFunction(_)))
            .unwrap();
        let wrong_origin = access_record(
            record,
            access.declared_visibility(),
            access.lexical_owners().to_vec(),
            other.declaration_access().definition_origin().clone(),
        );
        let mut reversed = access.lexical_owners().to_vec();
        reversed.reverse();
        let wrong_chain = access_record(
            record,
            access.declared_visibility(),
            reversed,
            access.definition_origin().clone(),
        );
        let foundation = fixture.bind().unwrap();
        assert!(
            matches!(foundation.bind_default_access_declarations(&replacing(table, wrong_origin), required),
            Err(Error::Origin(id)) if id == subject)
        );
        assert!(
            matches!(foundation.bind_default_access_declarations(&replacing(table, wrong_chain), required),
            Err(Error::Access { subject: id, error }) if id == subject && matches!(*error, hir::DeclarationAccessSourceSemanticError::OwnerChain))
        );
    });
}
#[test]
fn default_access_binding_checks_protected_class_and_existing_nominal_sources() {
    with_sources(SOURCE, |output, fixture, required, table| {
        let nominal = fixture
            .source
            .entries()
            .sources
            .records()
            .iter()
            .find(|r| r.access().lexical_owners().is_empty())
            .unwrap();
        let original = table.get(nominal_subject(nominal.owner())).unwrap();
        let changed = access_record(
            original,
            hir::DeclaredVisibilityV1::Internal,
            vec![],
            original.declaration_access().definition_origin().clone(),
        );
        let foundation = fixture.bind().unwrap();
        assert!(
            matches!(foundation.bind_default_access_declarations(&replacing(table, changed), required),
            Err(Error::NominalOverlap(id)) if id == nominal.owner())
        );
        let export = output.output().export.module();
        let (_, property) = export
            .properties
            .iter()
            .find(|(_, p)| matches!(p.owner, hir::PropertyOwner::Struct(_)))
            .unwrap();
        let subject = Subject::PropertyAccessor(
            export.property_accessor_identities[property.capability.getter()].id(),
        );
        let record = table.get(subject).unwrap();
        let access = record.declaration_access();
        let protected = access_record(
            record,
            hir::DeclaredVisibilityV1::Protected,
            access.lexical_owners().to_vec(),
            access.definition_origin().clone(),
        );
        assert!(
            matches!(foundation.bind_default_access_declarations(&replacing(table, protected), required),
            Err(Error::Access { subject: id, error }) if id == subject && matches!(*error, hir::DeclarationAccessSourceSemanticError::ProtectedOwnerNotClass))
        );
    });
}
#[test]
fn default_access_binding_rejects_const_getter_without_independent_source_demand() {
    with_sources(SOURCE, |output, fixture, _, table| {
        let export = output.output().export.module();
        let (id, property) = export
            .properties
            .iter()
            .find(|(_, p)| p.name == "fixed")
            .unwrap();
        let subject = Subject::PropertyAccessor(
            export.property_accessor_identities[property.capability.getter()].id(),
        );
        let property = Subject::Property(export.property_identities[id].ordinary_id().unwrap());
        let forged = Record::try_new(
            subject,
            table.get(property).unwrap().declaration_access().clone(),
        )
        .unwrap();
        // Internal const accessor identity and origin exist, but neither
        // creates an independent runtime accessor demand.
        assert!(export.export_definition_origins.get(subject).is_some());
        let table = Table::try_new(vec![forged]).unwrap();
        assert!(
            matches!(fixture.bind().unwrap().bind_default_access_declarations(&table, &BTreeSet::new()),
            Err(Error::UnexpectedRecord(id)) if id == subject)
        );
    });
}
