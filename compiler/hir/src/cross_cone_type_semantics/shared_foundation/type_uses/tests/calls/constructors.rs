use super::*;

mod negative;
mod resources;
mod roles;

fn construction(
    provider: ConeIdentity,
    owner: PersistentTypeId,
    target: CallableTemplateOrigin,
) -> SelectedExternalTypeUseV1 {
    let declaration = match target {
        CallableTemplateOrigin::Constructor(id) => SelectedTypeConstructionV1::Constructor(id),
        CallableTemplateOrigin::VariantConstructor(id) => {
            SelectedTypeConstructionV1::EnumVariant(id)
        }
        _ => panic!("construction fixtures require a constructor or variant"),
    };
    SelectedExternalTypeUseV1::new(
        provider,
        SelectedTypeUseV1::Construct {
            exact: exact(owner),
            declaration,
        },
    )
}

fn construct(
    provider: &mut Loaded,
    owner: PersistentTypeId,
    kind: SourceNominalKind,
) -> CallableTemplateOrigin {
    let parameters =
        vec![SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()); 2];
    if kind == SourceNominalKind::Enum {
        provider.variant(owner, "Entry", parameters)
    } else {
        provider.constructor(owner, parameters)
    }
}

#[test]
fn shared_source_construction_joins_class_struct_and_variant_calls_for_every_provider() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    for (coordinate, name) in [
        (ConeCoordinate::reserved_core(), "core"),
        (coordinate("provider"), "ordinary"),
    ] {
        for (kind, kind_name) in [
            (SourceNominalKind::Class, "class"),
            (SourceNominalKind::Struct, "struct"),
            (SourceNominalKind::Enum, "enum"),
        ] {
            let mut source = Artifact::new(coordinate.clone());
            let owner = source.nominal("Owner", kind, &[]);
            let mut provider = source.load(&[]);
            let target = construct(&mut provider, owner, kind);
            let dependencies = dependencies(&core, &provider);
            let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
            assert!(consumer.uses(&dependencies).unwrap().records().is_empty());
            consumer.declaration_calls(&provider, &[target]);
            let actual = consumer.uses(&dependencies).unwrap();
            let mut expected = signature_uses(provider.provider(), &[owner]);
            expected.push(construction(provider.provider(), owner, target));
            assert_eq!(actual, selected(expected));
            consumer
                .validate(&actual, &dependencies, &mut meter())
                .unwrap();
            let references = consumer
                .metadata()
                .public
                .external_references()
                .records()
                .to_vec();
            for reference in &references {
                for call in reference.call_sites().records() {
                    assert_eq!(
                        call.arguments(),
                        &[exact(CoreBuiltinNominal::Unit.identity_record().id()); 2]
                    );
                    assert_eq!(call.result(), exact(owner));
                }
                let decoded: DecodedExternalHirReferenceV1 = scoop_wire::decode_canonical(
                    &scoop_wire::encode(reference).unwrap(),
                    DecodeLimits::default(),
                )
                .unwrap();
                assert_eq!(
                    decoded.resolve(&mut consumer.identities).unwrap(),
                    *reference
                );
            }
            let decoded: DecodedCanonicalSelectedExternalTypeUsesV1 = scoop_wire::decode_canonical(
                &scoop_wire::encode(&actual).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap();
            assert_eq!(
                decoded
                    .resolve(&mut consumer.identities, &mut meter(), &WirePath::root())
                    .unwrap(),
                actual
            );
            snapshot(&format!("source-construction-{name}-{kind_name}"), &actual);
        }
    }
}

#[test]
fn shared_source_construction_combines_zero_argument_variants_members_and_foreign_zst_parameters() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("values"));
    let value = source.nominal("Value", SourceNominalKind::Struct, &[]);
    let values = source.load(&[&core]);
    let mut source = Artifact::new(coordinate("provider"));
    let base = source.nominal("Base", SourceNominalKind::Class, &[]);
    let owner = source.nominal("Owner", SourceNominalKind::Class, &[base]);
    let enumeration = source.nominal("Choice", SourceNominalKind::Enum, &[]);
    let mut provider = source.load(&[&core, &values]);
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let constructor = provider.constructor(
        owner,
        vec![
            SignatureTypeKey::Nominal(unit),
            SignatureTypeKey::Nominal(value),
            SignatureTypeKey::Nominal(unit),
        ],
    );
    let empty = provider.variant(enumeration, "Empty", Vec::new());
    let payload = provider.variant(
        enumeration,
        "Payload",
        vec![SignatureTypeKey::Nominal(value)],
    );
    let method = provider.callable(nominal_owner(owner), "inspect", CallForm::Function);
    let getter = provider.callable(nominal_owner(owner), "property", CallForm::Getter);
    let setter = provider.callable(nominal_owner(owner), "property", CallForm::Setter);
    let dependencies = vec![&core, &values, &provider];
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.declaration_calls(
        &provider,
        &[
            constructor,
            empty,
            fixture::origin(method),
            payload,
            fixture::origin(getter),
            constructor,
            fixture::origin(setter),
        ],
    );
    let actual = consumer.uses(&dependencies).unwrap();
    let mut expected = signature_uses(provider.provider(), &[owner, enumeration]);
    expected.extend([
        representation(provider.provider(), base),
        representation(values.provider(), value),
        signature(values.provider(), value),
        construction(provider.provider(), owner, constructor),
        construction(provider.provider(), enumeration, empty),
        construction(provider.provider(), enumeration, payload),
        member_call(provider.provider(), owner, method),
        member_call(provider.provider(), owner, getter),
        member_call(provider.provider(), owner, setter),
    ]);
    assert_eq!(actual, selected(expected));
    consumer
        .validate(&actual, &dependencies, &mut meter())
        .unwrap();
    snapshot("source-construction-combined", &actual);
}
