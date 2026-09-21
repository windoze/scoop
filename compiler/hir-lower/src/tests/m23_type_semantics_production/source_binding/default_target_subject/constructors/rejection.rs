use super::*;

#[test]
fn default_constructor_access_rejects_wrong_owner_root_kind_and_generic_arity() {
    for (source, cases) in [(SOURCE, CASES), (COMBINATIONS, COMBINED_CASES)] {
        with_hir_source(source, |output, _| {
            let fixture = Fixture::from_output(output);
            let foundation = fixture.bind().unwrap();
            for (name, position) in cases {
                let record = reference(output, name, *position);
                let target = record.target();
                let wrong = with_owner(
                    target,
                    SignatureTypeKey::Nominal(
                        scoop_identity::CoreBuiltinNominal::Any
                            .identity_record()
                            .id(),
                    ),
                );
                assert!(
                    matches!(
                        foundation.default_constructor_access_subject(&wrong, &mut meter()),
                        Err(Error::AppliedOwner(_))
                    ),
                    "{name}"
                );
                let wrong_kind = match target {
                    Constructor::Struct {
                        declaration,
                        owner_type,
                    } => Some(Constructor::Class {
                        declaration: ClassId::Source(*declaration),
                        owner_type: owner_type.clone(),
                    }),
                    Constructor::Class {
                        declaration: ClassId::Source(declaration),
                        owner_type,
                    } => Some(Constructor::Struct {
                        declaration: *declaration,
                        owner_type: owner_type.clone(),
                    }),
                    _ => None,
                };
                if let Some(wrong) = wrong_kind {
                    assert!(
                        matches!(
                            foundation.default_constructor_access_subject(&wrong, &mut meter()),
                            Err(Error::NominalKind { .. })
                        ),
                        "{name}"
                    );
                }
                if let SignatureTypeKey::NominalApplication { origin, arguments } =
                    target.owner_type()
                {
                    let wrong = with_owner(
                        target,
                        SignatureTypeKey::NominalApplication {
                            origin: *origin,
                            arguments: scoop_identity::NonEmptyVec::new(vec![
                                arguments.as_slice()[0].clone(),
                                arguments.as_slice()[0].clone(),
                            ])
                            .unwrap(),
                        },
                    );
                    assert!(
                        matches!(foundation.default_constructor_access_subject(&wrong, &mut meter()), Err(Error::AppliedOwnerArity { owner: SourceNominalId::GenericTemplate(id), expected: 1, actual: 2 }) if id == *origin)
                    );
                }
            }
        });
    }
}

#[test]
fn default_constructor_access_requires_actual_artifact_constructor_and_owner_keys() {
    for (source, cases) in [(SOURCE, CASES), (COMBINATIONS, COMBINED_CASES)] {
        with_hir_source(source, |output, _| {
            let fixture = Fixture::from_output(output);
            for (name, position) in cases {
                let record = reference(output, name, *position);
                for missing_owner in [false, true] {
                    let mut canonical = fixture.foundation.as_canonical().clone();
                    if missing_owner {
                        match record.target().owner_type() {
                            SignatureTypeKey::Nominal(_) => canonical.set_types(vec![]).unwrap(),
                            SignatureTypeKey::NominalApplication { .. } => {
                                canonical.set_generic_types(vec![]).unwrap()
                            }
                            _ => panic!("nominal owner"),
                        }
                    } else {
                        match record.target() {
                            Constructor::Variant { .. } => {
                                canonical.set_enum_variants(vec![]).unwrap()
                            }
                            _ => canonical.set_constructors(vec![]).unwrap(),
                        }
                    }
                    let artifact = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
                    let foundation = fixture
                        .source
                        .bind_to_foundation(&artifact, &fixture.identities, &mut meter())
                        .unwrap();
                    assert!(
                        matches!(
                            foundation
                                .default_constructor_access_subject(record.target(), &mut meter()),
                            Err(Error::MissingDeclaration(_)) | Err(Error::MissingTarget(_))
                        ),
                        "{name} missing_owner={missing_owner}"
                    );
                }
            }
        });
    }
}

#[test]
fn default_constructor_access_cannot_select_an_object_initialization_entry() {
    with_hir_source(SOURCE, |output, _| {
        let fixture = Fixture::from_output(output);
        let export = output.output().export.module();
        let (id, object) = export
            .objects
            .iter()
            .find(|(_, o)| o.name == "Registry")
            .unwrap();
        let source = export.constructor_identities
            [export.classes[object.backing_class].constructors[0]]
            .source_record()
            .unwrap()
            .id();
        let owner = export.nominal_identities[id].concrete_type_id().unwrap();
        let target = Constructor::Class {
            declaration: ClassId::Source(source),
            owner_type: SignatureTypeKey::Nominal(owner),
        };
        assert!(
            matches!(fixture.bind().unwrap().default_constructor_access_subject(&target, &mut meter()), Err(Error::NominalKind { owner: SourceNominalId::Concrete(actual), expected: scoop_identity::SourceDeclarationKind::Class }) if actual == owner)
        );
    });
}
