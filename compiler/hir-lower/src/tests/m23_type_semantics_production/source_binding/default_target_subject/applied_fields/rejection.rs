use super::*;

#[test]
fn applied_default_fields_reject_wrong_nominal_roles_roots_and_arities() {
    for (source, cases) in [(SOURCE, CASES), (COMBINATIONS, COMBINED_CASES)] {
        with_hir_source(source, |output, _| {
            let fixture = Fixture::from_output(output);
            let foundation = fixture.bind().unwrap();
            for (name, position) in &cases[..3] {
                let record = reference(output, name, *position);
                let target = record.target();
                let wrong = with_owner(
                    target,
                    SignatureTypeKey::Nominal(CoreBuiltinNominal::Any.identity_record().id()),
                );
                assert!(
                    matches!(
                        foundation.default_field_access_subject(&wrong),
                        Err(Error::AppliedOwner(_))
                    ),
                    "{name}"
                );
                let (wrong_kind, owner) = match target {
                    Field::Struct {
                        declaration,
                        owner_type,
                    } => (
                        Field::Class {
                            declaration: *declaration,
                            owner_type: owner_type.clone(),
                        },
                        owner_type,
                    ),
                    Field::Class {
                        declaration,
                        owner_type,
                    } => (
                        Field::Struct {
                            declaration: *declaration,
                            owner_type: owner_type.clone(),
                        },
                        owner_type,
                    ),
                    Field::Tuple { .. } => panic!("declared field"),
                };
                assert!(
                    matches!(
                        foundation.default_field_access_subject(&wrong_kind),
                        Err(Error::Role(_))
                    ),
                    "{name}"
                );
                if let SignatureTypeKey::NominalApplication { origin, arguments } = owner {
                    let wrong = with_owner(
                        target,
                        SignatureTypeKey::NominalApplication {
                            origin: *origin,
                            arguments: scoop_identity::NonEmptyVec::new(vec![
                                arguments.as_slice()
                                    [0]
                                .clone();
                                2
                            ])
                            .unwrap(),
                        },
                    );
                    assert!(
                        matches!(
                            foundation.default_field_access_subject(&wrong),
                            Err(Error::AppliedOwnerArity {
                                expected: 1,
                                actual: 2,
                                ..
                            })
                        ),
                        "{name}"
                    );
                }
            }
        });
    }
}

#[test]
fn applied_default_fields_require_artifact_owned_field_and_declaration_records() {
    for (source, cases) in [(SOURCE, CASES), (COMBINATIONS, COMBINED_CASES)] {
        with_hir_source(source, |output, _| {
            let fixture = Fixture::from_output(output);
            for (name, position) in &cases[..3] {
                let record = reference(output, name, *position);
                for missing_declaration in [false, true] {
                    let mut canonical = fixture.foundation.as_canonical().clone();
                    if missing_declaration {
                        match record.target() {
                            Field::Struct {
                                owner_type: SignatureTypeKey::Nominal(_),
                                ..
                            } => canonical.set_types(vec![]).unwrap(),
                            Field::Struct { .. } => canonical.set_generic_types(vec![]).unwrap(),
                            Field::Class { .. } => canonical.set_properties(vec![]).unwrap(),
                            Field::Tuple { .. } => panic!("declared field"),
                        }
                    } else {
                        canonical.set_fields(vec![]).unwrap();
                    }
                    let artifact = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
                    let foundation = fixture
                        .source
                        .bind_to_foundation(&artifact, &fixture.identities)
                        .unwrap();
                    assert!(
                        matches!(
                            foundation.default_field_access_subject(record.target()),
                            Err(Error::MissingDeclaration(_)) | Err(Error::MissingTarget(_))
                        ),
                        "{name}"
                    );
                }
            }
        });
    }
}

#[test]
fn object_default_field_owner_is_the_source_object_not_its_backing_nominal() {
    for (source, cases) in [(SOURCE, CASES), (COMBINATIONS, COMBINED_CASES)] {
        with_hir_source(source, |output, _| {
            let fixture = Fixture::from_output(output);
            let export = output.output().export.module();
            let record = reference(output, cases[2].0, cases[2].1);
            let object = export
                .objects
                .iter()
                .find(|(_, o)| o.name.ends_with("Cache"))
                .unwrap()
                .1;
            let backing_key = export.nominal_identities[object.backing_class]
                .generated()
                .unwrap()
                .key();
            let backing = PersistentTypeId::from_generated_key(backing_key).unwrap();
            let wrong = with_owner(record.target(), SignatureTypeKey::Nominal(backing));
            assert!(matches!(
                fixture.bind().unwrap().default_field_access_subject(&wrong),
                Err(Error::AppliedOwner(_))
            ));
            let mut canonical = fixture.foundation.as_canonical().clone();
            canonical.set_generated_types(vec![]).unwrap();
            let artifact = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let error = match fixture
                .source
                .bind_to_foundation(&artifact, &fixture.identities)
            {
                Ok(foundation) => foundation
                    .default_field_access_subject(record.target())
                    .unwrap_err(),
                Err(error) => Error::Foundation(error),
            };
            assert!(
                matches!(error, Error::MissingGenerated(id) | Error::Foundation(hir::TypeFoundationBindingError::MissingGenerated(id)) if id == backing)
            );
        });
    }
}
