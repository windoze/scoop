use super::*;

#[test]
fn shared_source_construction_rejects_missing_extra_and_misdirected_selections() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    for kind in [
        SourceNominalKind::Class,
        SourceNominalKind::Struct,
        SourceNominalKind::Enum,
    ] {
        let mut source = Artifact::new(coordinate("provider"));
        let owner = source.nominal("Owner", kind, &[]);
        let other = source.nominal("Other", kind, &[]);
        let mut provider = source.load(&[&core]);
        let target = construct(&mut provider, owner, kind);
        let unused = construct(&mut provider, other, kind);
        let alternative = if kind == SourceNominalKind::Enum {
            provider.variant(owner, "Alternative", Vec::new())
        } else {
            provider.constructor(owner, Vec::new())
        };
        let dependencies = dependencies(&core, &provider);
        let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
        consumer.declaration_calls(&provider, &[target]);
        let actual = consumer.uses(&dependencies).unwrap();
        let position = actual
            .records()
            .iter()
            .position(|record| *record == construction(provider.provider(), owner, target))
            .unwrap();
        let mut missing = actual.records().to_vec();
        missing.remove(position);
        assert!(matches!(
            consumer.validate(&selected(missing), &dependencies, &mut meter()),
            Err(Error::TypeUseInventory)
        ));
        for replacement in [
            construction(core.provider(), owner, target),
            construction(provider.provider(), other, target),
            construction(provider.provider(), owner, unused),
            construction(provider.provider(), owner, alternative),
        ] {
            let mut changed = actual.records().to_vec();
            changed[position] = replacement;
            assert!(matches!(
                consumer.validate(&selected(changed), &dependencies, &mut meter()),
                Err(Error::TypeUseInventory)
            ));
        }
        let mut extra = actual.records().to_vec();
        extra.push(construction(provider.provider(), owner, alternative));
        assert!(matches!(
            consumer.validate(&selected(extra), &dependencies, &mut meter()),
            Err(Error::TypeUseInventory)
        ));
        assert!(
            matches!(consumer.uses(&[&core]), Err(Error::MissingProvider(origin)) if origin == provider.provider())
        );
    }
}

#[test]
fn shared_source_construction_checks_every_logical_argument_and_result_at_the_actual_position() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let unit = exact(CoreBuiltinNominal::Unit.identity_record().id());
    for kind in [
        SourceNominalKind::Class,
        SourceNominalKind::Struct,
        SourceNominalKind::Enum,
    ] {
        let mut source = Artifact::new(coordinate("provider"));
        let owner = source.nominal("Owner", kind, &[]);
        let mut provider = source.load(&[&core]);
        let target = construct(&mut provider, owner, kind);
        let dependencies = dependencies(&core, &provider);
        for (case, arguments, result) in [
            (0, vec![unit], exact(owner)),
            (1, vec![unit, exact(owner)], exact(owner)),
            (2, vec![unit; 2], unit),
            (3, vec![exact(owner), unit, unit], exact(owner)),
        ] {
            let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
            consumer.declaration_calls(&provider, &[target, target]);
            consumer.change_last_call(|site| {
                HirDependencyCallSiteV1::try_new(
                    site.position(),
                    site.origin().clone(),
                    arguments,
                    result,
                    site.witness_indices().to_vec(),
                    site.receiver(),
                )
                .unwrap()
            });
            let Err(Error::CallSignature { position, source }) = consumer.uses(&dependencies)
            else {
                panic!("invalid constructor signature must fail before demand deduplication");
            };
            assert_eq!(position.expression_index, 1);
            assert!(
                matches!(
                    (case, source.as_ref()),
                    (
                        0,
                        HirDependencyCallSignatureError::ArgumentCount {
                            expected: 2,
                            actual: 1
                        }
                    ) | (
                        1,
                        HirDependencyCallSignatureError::Argument { index: 1, .. }
                    ) | (2, HirDependencyCallSignatureError::Result { .. })
                        | (
                            3,
                            HirDependencyCallSignatureError::ArgumentCount {
                                expected: 2,
                                actual: 3
                            }
                        )
                ),
                "unexpected constructor signature diagnostic: {source}"
            );
            let message = source.to_string();
            assert!(message.contains(if case == 2 {
                "result exact type"
            } else if case == 1 {
                "argument 1"
            } else {
                "logical arguments"
            }));
        }
    }
}

#[test]
fn shared_source_construction_requires_positioned_source_calls_with_binding_witnesses() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    for kind in [SourceNominalKind::Class, SourceNominalKind::Enum] {
        let mut source = Artifact::new(coordinate("provider"));
        let owner = source.nominal("Owner", kind, &[]);
        let mut provider = source.load(&[&core]);
        let target = construct(&mut provider, owner, kind);
        let dependencies = dependencies(&core, &provider);
        let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
        consumer.declaration_calls(&provider, &[target]);
        let reference = consumer
            .metadata()
            .public
            .external_references()
            .records()
            .iter()
            .find(|reference| reference.target() == ExternalHirTargetV1::Callable(target))
            .unwrap();
        assert_eq!(
            ExternalHirReferenceV1::try_new(
                reference.origin(),
                reference.target(),
                reference.roles().clone(),
                reference.witnesses().clone(),
                Default::default(),
                reference.type_sites().clone()
            ),
            Err(ExternalHirReferenceBuildError::MissingCallSites)
        );
        assert_eq!(
            ExternalHirReferenceV1::try_new(
                reference.origin(),
                reference.target(),
                reference.roles().clone(),
                CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap(),
                reference.call_sites().clone(),
                reference.type_sites().clone()
            ),
            Err(ExternalHirReferenceBuildError::MissingWitness)
        );
    }
}
