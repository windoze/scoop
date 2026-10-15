use super::*;

pub(super) fn foundation(
    name: &str,
    body: bool,
) -> (StrongCallableDefinitionOwner, ConeLirFoundation) {
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
    let (definition, symbol) = ExternalStrongShapeSubjectV1::Callable(
        scoop_identity::CallableDefinitionOwner::Strong(target),
    )
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
        ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap(),
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
        value: value.value(),
    }];
    let bound = Bound::value(exact(&owner));
    ExactValueLayoutV1::ordinary_struct(
        bound.identity,
        false,
        if zst { &[] } else { &input },
        &bound.foundation,
    )
    .unwrap()
    .into()
}

pub(super) fn function(
    exact_result: PersistentExactTypeId,
    result: ScoopAbiReturn,
    arguments: Vec<ScoopAbiArgument>,
) -> (ExactCallableAbiExportV1, ConeLirFoundation) {
    let signature = ExactCallableSignature::new(
        Effect::Ordinary,
        None,
        arguments
            .iter()
            .map(|argument| argument.storage().exact_type())
            .collect(),
        exact_result,
    );
    let (target, foundation) = foundation("function", true);
    let record = ExactCallableAbiExportV1::from_signature(
        TARGET,
        target,
        CanonicalScoopAbiFunctionSignature::new(
            signature,
            arguments,
            result,
            scoop_identity::GcEffect::NoGc,
        )
        .unwrap(),
        &foundation,
    )
    .unwrap();
    (record, foundation)
}

pub(super) fn integer_part(value: CanonicalScoopStorage, bits: u8) -> AbiCoercion {
    AbiCoercion::One(
        AbiPart::new(
            AbiCarrier::Integer(bits),
            0,
            value.byte_size(),
            value.alignment().get(),
        )
        .unwrap(),
    )
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
        value: payload.value(),
        pointer_kind: match payload.representation().kind() {
            ExactRepresentationKindV1::QualifiedPointer(kind) => Some(kind),
            _ => None,
        },
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
    ExactValueLayoutV1::enumeration(bound.identity, &inputs, &bound.foundation)
        .unwrap()
        .into()
}

pub(super) fn roundtrip(expected: &ExactCallableAbiExportV1, foundation: &ConeLirFoundation) {
    let bytes = encode(expected).unwrap();
    let raw = decode_canonical::<DecodedExactCallableAbiExportV1>(&bytes).unwrap();
    assert_eq!(bytes[0], 0xa5);
    assert_eq!(encode(&raw).unwrap(), bytes);
    let mut graph = identities(foundation, std::slice::from_ref(expected));
    assert_eq!(raw.read(TARGET, foundation, &mut graph).unwrap(), *expected);
}

pub(super) fn identities(
    foundation: &ConeLirFoundation,
    records: &[ExactCallableAbiExportV1],
) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(foundation.producer()).unwrap();
    let mut exacts = std::collections::BTreeSet::new();
    for record in records {
        let CallableDefinitionOwner::Strong(StrongCallableDefinitionOwner::Function(function)) =
            record.target()
        else {
            panic!("ABI unit fixtures use source functions");
        };
        pending.register_authority(function).unwrap();
        let signature = record.canonical_signature().signature();
        exacts.extend(signature.receiver().into_option());
        exacts.extend(signature.parameters().iter().copied());
        exacts.insert(signature.result());
    }
    for exact in exacts {
        pending.register_authority(exact).unwrap();
    }
    for plan in foundation.definition_plans() {
        pending.register_authority(plan.id()).unwrap();
    }
    let bodies = foundation
        .callable_bodies()
        .iter()
        .map(|record| {
            decode_canonical::<DecodedRuntimeIdentityRecord<PersistentCallableBodyId>>(
                &encode(record).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    for body in &bodies {
        pending
            .register_callable_body(IdentityLayer::Lir, body)
            .unwrap();
    }
    for body in &bodies {
        pending.resolve_callable_body(body).unwrap();
    }
    pending.finish().unwrap()
}
