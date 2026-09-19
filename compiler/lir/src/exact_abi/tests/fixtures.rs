use super::*;

pub(super) fn foundation(
    name: &str,
    body: bool,
) -> (StrongCallableDefinitionOwner, OdrFreeLirFoundation) {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let target = StrongCallableDefinitionOwner::Function(
        PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            vec![],
        ))
        .unwrap(),
    );
    let (definition, symbol) = ExternalStrongShapeSubjectV1::Callable(target)
        .expected_definition(ConeIdentity::SINGLE_FILE)
        .unwrap();
    let definition = CborIdentityRecord::from_key(definition).unwrap();
    let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(vec![definition]).unwrap();
    canonical.set_definition_atoms(vec![atom]).unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(vec![
            PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap(),
        ])
        .unwrap(),
    );
    if body {
        canonical
            .set_callable_bodies(vec![
                RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(target)).unwrap(),
            ])
            .unwrap();
    }
    (
        target,
        OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap(),
    )
}

pub(super) fn aggregate(zst: bool) -> ExactLayoutExportV1 {
    let owner = source(
        if zst { "Empty" } else { "Struct" },
        SourceNominalKind::Struct,
        0,
    );
    let value = integer("Byte", IntegerKind::SIGNED_8);
    let field = field(&owner, "byte");
    let input = [NominalLayoutFieldInputV1 {
        field: &field,
        value: &value,
    }];
    let bound = Bound::value(exact(&owner));
    ExactValueLayoutV1::ordinary_struct(
        bound.identity,
        false,
        if zst { &[] } else { &input },
        &bound.foundation,
        &mut meter(),
    )
    .unwrap()
    .into()
}

pub(super) fn function(
    result: &ExactLayoutExportV1,
    parameters: &[&ExactLayoutExportV1],
) -> ExactCallableAbiExportV1 {
    let signature = ExactCallableSignature::new(
        Effect::Ordinary,
        None,
        parameters
            .iter()
            .map(|layout| layout.identity().exact())
            .collect(),
        result.identity().exact(),
    );
    let (target, foundation) = foundation("function", true);
    ExactCallableAbiExportV1::replay(
        TARGET,
        target,
        signature,
        ExactCallableProtocolV1::OrdinaryNoGc,
        CallableAbiLayoutInputsV1 {
            receiver: CallableAbiReceiverInputV1::NoReceiver,
            parameters,
            result,
        },
        &foundation,
        &mut meter(),
    )
    .unwrap()
}

pub(super) fn enumeration(payload: &ExactValueLayoutV1) -> ExactLayoutExportV1 {
    let owner = source("Optional", SourceNominalKind::Enum, 0);
    let variants: Vec<_> = ["Empty", "Value"]
        .iter()
        .map(|name| {
            CborIdentityRecord::from_key(
                EnumVariantIdentityKey::source(&owner, CanonicalIdentifier::new(name).unwrap())
                    .unwrap(),
            )
            .unwrap()
        })
        .collect();
    let field = CborIdentityRecord::from_key(EnumVariantFieldKey::new(
        variants[1].id(),
        EnumVariantFieldSelector::Positional {
            declaration_index: 0,
        },
    ))
    .unwrap();
    let fields = [EnumLayoutFieldInputV1 {
        field: &field,
        value: payload,
    }];
    let inputs = [
        EnumLayoutVariantInputV1 {
            variant: &variants[0],
            fields: &[],
        },
        EnumLayoutVariantInputV1 {
            variant: &variants[1],
            fields: &fields,
        },
    ];
    let bound = Bound::value(exact(&owner));
    ExactValueLayoutV1::enumeration(bound.identity, &inputs, &bound.foundation, &mut meter())
        .unwrap()
        .into()
}

pub(super) fn roundtrip(expected: &ExactCallableAbiExportV1) {
    let bytes = encode(expected).unwrap();
    let raw = decode_canonical::<DecodedExactCallableAbiExportV1>(&bytes, DecodeLimits::default())
        .unwrap();
    assert_eq!(bytes[0], 0xa6);
    assert_eq!(encode(&raw).unwrap(), bytes);
    assert_eq!(
        raw.validate_against(expected, &mut meter()).unwrap(),
        *expected
    );
}
