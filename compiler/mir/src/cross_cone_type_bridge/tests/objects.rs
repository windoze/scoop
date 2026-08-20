use super::*;

fn reference_facts() -> MirTypeFactsV1 {
    facts(
        MirValueKindV1::Reference,
        MirGcKindV1::ContainsManagedReferences,
    )
}

#[test]
fn object_backing_retains_hir_identity_and_can_inherit_a_source_class() {
    let mut fixture = Fixture::new();
    let relation = MirBaseAndInterfacesV1 {
        base: MirBaseClassV1::Base(exact(fixture.class.id()).id()),
        interfaces: vec![exact(fixture.interface.id()).id()],
    };
    let backing = ParamFreeMirTypeExportV1::try_new(
        fixture.authority(),
        exact(fixture.backing.id()).id(),
        MirTypeOriginV1::GeneratedNominal {
            nominal: fixture.backing.id(),
            role: fixture.backing.key().clone(),
        },
        reference_facts(),
        MirTypeRepresentationV1::ObjectBacking {
            declared_fields: vec![],
        },
        relation.clone(),
    )
    .unwrap();
    assert!(
        fixture
            .foundation
            .as_canonical()
            .generated_type_key(fixture.backing.id())
            .is_none(),
        "HIR-owned backing classes are not MIR-generated helpers"
    );
    let object = ParamFreeMirTypeExportV1::try_new(
        fixture.authority(),
        exact(fixture.object.id()).id(),
        MirTypeOriginV1::SourceNominal(fixture.object.id()),
        reference_facts(),
        MirTypeRepresentationV1::Object {
            backing: backing.exact(),
        },
        relation,
    )
    .unwrap();
    let expected = CanonicalParamFreeMirTypeExportsV1::try_new(vec![object, backing]).unwrap();
    let decoded: DecodedCanonicalParamFreeMirTypeExportsV1 =
        decode_canonical(&encode(&expected).unwrap()).unwrap();
    assert_eq!(
        decoded
            .validate(&mut fixture.graph, &fixture.foundation)
            .unwrap(),
        expected
    );
}

#[test]
fn object_cannot_alias_a_source_class_or_a_different_generated_family() {
    let fixture = Fixture::new();
    for backing in [
        exact(fixture.class.id()).id(),
        exact(fixture.boxed.generated_type_record().id()).id(),
    ] {
        assert!(matches!(
            fixture.source(
                fixture.object.id(),
                reference_facts(),
                MirTypeRepresentationV1::Object { backing },
            ),
            Err(MirTypeBridgeError::ExactOriginMismatch { .. })
        ));
    }
    assert!(matches!(
        ParamFreeMirTypeExportV1::try_new(
            fixture.authority(),
            exact(fixture.backing.id()).id(),
            MirTypeOriginV1::SourceNominal(fixture.backing.id()),
            reference_facts(),
            MirTypeRepresentationV1::Class {
                release_policy: Default::default(),
                kind: MirClassKindV1::Final,
                declared_fields: vec![],
            },
            no_bases(),
        ),
        Err(MirTypeBridgeError::Reference(_))
    ));
}

#[test]
fn execution_helpers_are_rejected_even_with_complete_canonical_identities() {
    let mut fixture = Fixture::new();
    let exact = exact(fixture.frame.id()).id();
    let origin = MirTypeOriginV1::GeneratedNominal {
        nominal: fixture.frame.id(),
        role: fixture.frame.key().clone(),
    };
    let representation = MirTypeRepresentationV1::Class {
        release_policy: Default::default(),
        kind: MirClassKindV1::Final,
        declared_fields: vec![],
    };
    assert!(matches!(
        ParamFreeMirTypeExportV1::try_new(
            fixture.authority(),
            exact,
            origin.clone(),
            reference_facts(),
            representation.clone(),
            no_bases(),
        ),
        Err(MirTypeBridgeError::GeneratedExecutionShapeGate { .. })
    ));
    // Construct the complete untrusted product without creating a validated export.
    let mut bytes = vec![0xa5, 1];
    bytes.extend(encode(&exact).unwrap());
    bytes.push(2);
    bytes.extend(encode(&origin).unwrap());
    bytes.push(3);
    bytes.extend(encode(&reference_facts()).unwrap());
    bytes.push(4);
    bytes.extend(encode(&representation).unwrap());
    bytes.push(5);
    bytes.extend(encode(&no_bases()).unwrap());
    let decoded: DecodedParamFreeMirTypeExportV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.validate(&mut fixture.graph, &fixture.foundation),
        Err(MirTypeBridgeError::GeneratedExecutionShapeGate { .. })
    ));
}
