use super::*;
use scoop_identity::{
    CborIdentityRecord, DecodedCborIdentityRecord, DecodedFieldIdentityKey, FieldIdentityKey,
    GeneratedNominalKey, IdentityLayer, PendingIdentityValidation, PersistentFieldId,
    PersistentPropertyId, PersistentTypeId,
};

type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;

fn extend_graph(fixture: &Fixture, record: &FieldRecord) -> ValidatedIdentityGraph {
    let decoded: DecodedCborIdentityRecord<PersistentFieldId, DecodedFieldIdentityKey> =
        decode_canonical(&encode(record).unwrap()).unwrap();
    let mut validation = PendingIdentityValidation::new();
    validation
        .register_external_graph_authorities(&fixture.identities)
        .unwrap();
    validation.register(IdentityLayer::Hir, &decoded).unwrap();
    validation.resolve(&decoded).unwrap();
    validation.finish().unwrap()
}

fn misplaced_fields(export: &hir::ExportHir) -> Vec<FieldRecord> {
    let (_, class) = export
        .classes
        .iter()
        .find(|(_, c)| c.name == "Cell")
        .unwrap();
    let cell = &export.class_fields[class.fields[0]];
    let class_key = export.nominal_identities[cell.owner]
        .source()
        .unwrap()
        .declaration();
    let (object_id, object) = export
        .objects
        .iter()
        .find(|(_, o)| o.name == "Cache")
        .unwrap();
    let object_field = &export.class_fields[export.classes[object.backing_class].fields[0]];
    let property = |id| -> PersistentPropertyId {
        let hir::HirPropertyIdentity::Ordinary(record) = &export.property_identities[id] else {
            panic!("ordinary property");
        };
        record.id()
    };
    let object_key = export.nominal_identities[object_id]
        .source()
        .unwrap()
        .declaration();
    let backing = GeneratedNominalKey::ObjectBackingClass {
        object: PersistentTypeId::from_source_declaration(object_key).unwrap(),
    };
    [
        FieldIdentityKey::source_property_backing(class_key, property(object_field.property))
            .unwrap(),
        FieldIdentityKey::source_property_delegate(class_key, property(object_field.property))
            .unwrap(),
        FieldIdentityKey::object_backing_property(&backing, property(cell.property)).unwrap(),
    ]
    .into_iter()
    .map(|key| FieldRecord::from_key(key).unwrap())
    .collect()
}

#[test]
fn default_field_routes_require_one_identity_graph_and_matching_logical_property_owner() {
    with_hir_source(SOURCE, |output, _| {
        let fixture = Fixture::from_output(output);
        for record in misplaced_fields(output.output().export.module()) {
            let target = Target::ClassField(record.id());
            let mut canonical = fixture.foundation.as_canonical().clone();
            canonical.set_fields(vec![record.clone()]).unwrap();
            let artifact = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let foundation = fixture
                .source
                .bind_to_foundation(&artifact, &fixture.identities)
                .unwrap();
            assert!(matches!(
                foundation.default_indirect_access_subject(target),
                Err(Error::Foundation(
                    hir::TypeFoundationBindingError::Identity(_)
                ))
            ));
            let graph = extend_graph(&fixture, &record);
            let foundation = fixture
                .source
                .bind_to_foundation(&artifact, &graph)
                .unwrap();
            assert!(
                matches!(foundation.default_indirect_access_subject(target), Err(Error::PropertyOwner { field, .. }) if field == record.id())
            );
        }
    });
}

#[test]
fn default_object_fields_require_the_artifacts_generated_backing_relation() {
    with_hir_source(SOURCE, |output, _| {
        let fixture = Fixture::from_output(output);
        let export = output.output().export.module();
        let (field, owner) = export
            .class_fields
            .iter()
            .find_map(|(id, _)| {
                let record = &export.field_identities[id];
                if let scoop_identity::FieldIdentityView::Generated { owner, key } =
                    record.key().view()
                    && key.object_backing_property().is_some()
                {
                    Some((record.id(), owner))
                } else {
                    None
                }
            })
            .unwrap();
        let mut canonical = fixture.foundation.as_canonical().clone();
        canonical.set_generated_types(vec![]).unwrap();
        let artifact = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let error = match fixture
            .source
            .bind_to_foundation(&artifact, &fixture.identities)
        {
            Ok(foundation) => foundation
                .default_indirect_access_subject(Target::ClassField(field))
                .unwrap_err(),
            Err(error) => Error::Foundation(error),
        };
        assert!(
            matches!(error, Error::MissingGenerated(id) | Error::Foundation(hir::TypeFoundationBindingError::MissingGenerated(id)) if id == owner)
        );
    });
}

#[test]
fn default_field_routes_cannot_borrow_foreign_nominal_ownership_from_the_artifact() {
    with_hir_source(SOURCE, |output, _| {
        let fixture = Fixture::from_output(output);
        let property = expected::targets(output.output().export.module())
            .into_iter()
            .find_map(|(_, subject, _)| match subject {
                Subject::Property(id) => Some(id),
                _ => None,
            })
            .unwrap();
        let foreign = scoop_identity::CoreBuiltinNominal::Any.identity_record();
        let record = FieldRecord::from_key(
            FieldIdentityKey::source_property_backing(foreign.key(), property).unwrap(),
        )
        .unwrap();
        let graph = extend_graph(&fixture, &record);
        let mut canonical = fixture.foundation.as_canonical().clone();
        canonical.set_fields(vec![record.clone()]).unwrap();
        canonical.set_types(vec![foreign.clone()]).unwrap();
        let artifact = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let foundation = fixture
            .source
            .bind_to_foundation(&artifact, &graph)
            .unwrap();
        assert!(
            matches!(foundation.default_indirect_access_subject(Target::ClassField(record.id())), Err(Error::ForeignDeclaration(Subject::Type(id))) if id == foreign.id())
        );
    });
}
