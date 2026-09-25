use super::*;
use scoop_identity::NonEmptyVec;

#[test]
fn external_nominal_references_round_trip_for_core_and_ordinary_consumers() {
    let provider = ConeCoordinate::new("example", "provider", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    for coordinate in consumer_coordinates() {
        let (canonical, _, _) = external_nominals(provider);
        let decoded = decode(&canonical);
        let mut identities = validate_identities(
            &decoded,
            [ConeIdentity::CORE, coordinate.identity().unwrap(), provider],
        );
        let validated = decoded.validate(&coordinate, &mut identities).unwrap();
        assert_eq!(encode(&validated).unwrap(), encode(&canonical).unwrap());
        assert_eq!(validated.counts().types, 2);
        assert_eq!(validated.counts().generic_types, 0);
        assert_eq!(validated.counts().external_source_types, 1);
        assert_eq!(validated.counts().external_generic_types, 1);
    }
}

#[test]
fn every_consumer_rejects_unused_external_nominal_references() {
    let provider = ConeCoordinate::new("example", "unused-provider", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    for coordinate in consumer_coordinates() {
        for generic in [false, true] {
            let (mut canonical, source, template) = external_nominals(provider);
            let retained = canonical
                .exact_types
                .iter()
                .filter(|record| match record.key() {
                    ExactTypeKey::Nominal(id) => generic || *id != source.id(),
                    ExactTypeKey::NominalApplication { origin, .. } => {
                        !generic || *origin != template.id()
                    }
                    _ => true,
                })
                .cloned()
                .collect();
            canonical.set_exact_types(retained).unwrap();
            let decoded = decode(&canonical);
            let mut identities = validate_identities(
                &decoded,
                [ConeIdentity::CORE, coordinate.identity().unwrap(), provider],
            );
            let error = decoded
                .validate(&coordinate, &mut identities)
                .err()
                .expect("unused external references must fail shared validation");
            match error {
                HirFoundationValidationError::InvalidExternalSourceType(id) if !generic => {
                    assert_eq!(id, source.id());
                }
                HirFoundationValidationError::InvalidExternalGenericType(id) if generic => {
                    assert_eq!(id, template.id());
                }
                other => panic!("unexpected external-reference error: {other}"),
            }
        }
    }
}

#[test]
fn every_consumer_rejects_local_nominals_in_the_external_reference_table() {
    for coordinate in consumer_coordinates() {
        for generic in [false, true] {
            let (mut canonical, source, template) =
                external_nominals(coordinate.identity().unwrap());
            if generic {
                canonical.set_generic_types(vec![template.clone()]).unwrap();
            } else {
                canonical
                    .set_types(vec![
                        CoreBuiltinNominal::Unit.identity_record(),
                        CoreBuiltinNominal::Any.identity_record(),
                        source.clone(),
                    ])
                    .unwrap();
            }
            let decoded = decode(&canonical);
            let mut pending = PendingIdentityValidation::new();
            let authorities = [ConeIdentity::CORE, coordinate.identity().unwrap()]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            for authority in authorities {
                pending.register_authority(authority).unwrap();
            }
            let expected = if generic {
                ("generic type", *template.id().as_array())
            } else {
                ("type", *source.id().as_array())
            };
            assert!(matches!(
                decoded.register_identities(&mut pending),
                Err(IdentityValidationError::DuplicateIdentity { kind, id })
                    if (kind, id) == expected
            ));
        }
    }
}

fn consumer_coordinates() -> [ConeCoordinate; 2] {
    [
        ConeCoordinate::reserved_core(),
        ConeCoordinate::new("example", "consumer", "0.1.0").unwrap(),
    ]
}

fn external_nominals(
    provider: ConeIdentity,
) -> (CanonicalHirFoundation, TypeRecord, GenericTypeRecord) {
    let source = TypeRecord::from_key(declaration(provider, "Value")).unwrap();
    let generic = GenericTypeRecord::from_key(SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Container").unwrap(),
        SourceNominalKind::Struct,
        1,
    ))
    .unwrap();
    let unit = ExactTypeRecord::from_key(ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap();
    let source_exact = ExactTypeRecord::from_key(ExactTypeKey::Nominal(source.id())).unwrap();
    let generic_exact = ExactTypeRecord::from_key(ExactTypeKey::NominalApplication {
        origin: generic.id(),
        arguments: NonEmptyVec::new(vec![unit.id()]).unwrap(),
    })
    .unwrap();
    let mut canonical = CanonicalHirFoundation::empty();
    canonical
        .set_types(vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
        ])
        .unwrap();
    canonical
        .set_exact_types(vec![unit, source_exact, generic_exact])
        .unwrap();
    canonical
        .set_external_source_types(vec![source.id()])
        .unwrap();
    canonical
        .set_external_generic_types(vec![generic.id()])
        .unwrap();
    (canonical, source, generic)
}
