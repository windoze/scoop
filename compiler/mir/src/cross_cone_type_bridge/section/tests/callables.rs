use super::identity_support::{add_function, update};
use super::*;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionOwnerAtom,
    DefinitionOwnerChain, ExactCallableSignature, PackagePath, PersistentFunctionId,
    SourceDeclarationKey, SourceDeclarationSite,
};

fn value_method(fixture: &mut Fixture) -> StrongCallableDefinitionOwner {
    let method: CborIdentityRecord<PersistentFunctionId, _> =
        CborIdentityRecord::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                fixture.source.provider,
                PackagePath::root(),
                DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(
                    fixture.types.empty.id(),
                )]),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("identity").unwrap(),
            0,
            None,
            vec![],
        ))
        .unwrap();
    let target = StrongCallableDefinitionOwner::Function(method.id());
    let signature = ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        Some(fixture.types.payload.id()),
        vec![],
        fixture.types.payload.id(),
    );
    let mut hir = scoop_hir::CanonicalHirFoundation::empty();
    hir.set_functions(vec![method.clone()]).unwrap();
    let mut mir = fixture.types.foundation.as_canonical().clone();
    mir.set_callable_signatures(vec![crate::CallableSignatureRecord::new(
        crate::CallableSignatureSubject::strong(target.callable_owner()),
        signature.clone(),
    )])
    .unwrap();
    update(&mut fixture.types, hir, mir, &[]);
    fixture.production = production(fixture.source.provider, &fixture.types.foundation);
    let signature = MirBridgeCallableSignatureV1::new(signature, crate::GcEffect::NoGc);
    let method = ParamFreeMirCallableBindingV1::try_new(
        MirCallableBridgeAuthority {
            identities: &fixture.types.graph,
            foundation: &fixture.types.foundation,
            types: fixture.source.expected.types(),
        },
        MirCallableOriginV1::Function(method.id()),
        target,
        signature.clone(),
        signature,
        MirCallableLoweringRoleV1::Ordinary,
    )
    .unwrap();
    let callables = CanonicalMirCallableBindingsV1::try_new(vec![method]).unwrap();
    let authority = MirDispatchSchemaAuthority {
        identities: &fixture.types.graph,
        types: fixture.source.expected.types(),
        callables: &callables,
    };
    let dispatch = ParamFreeMirDispatchSchemaV1::try_new(
        authority,
        fixture.types.payload.id(),
        MirClassVtableSchemaV1::NoClassVtable,
        vec![],
        &mut meter(),
    )
    .unwrap();
    let dispatch =
        CanonicalMirDispatchSchemasV1::try_new(authority, vec![dispatch], &mut meter()).unwrap();
    let previous = &fixture.source.expected;
    fixture.source.expected = MirTypeBridgeExportConstituentsV1::new(
        previous.types().clone(),
        callables,
        dispatch,
        previous.objects().clone(),
        previous.shapes().clone(),
        previous.initialization_uses().clone(),
    );
    fixture.source.callables = vec![target];
    fixture.source.dispatch = vec![fixture.types.payload.id()];
    target
}

#[test]
fn value_method_selection_closes_both_signatures_and_preserves_gc_effect() {
    let mut provider = Fixture::new("method-provider");
    let target = value_method(&mut provider);
    let mut consumer = Fixture::new("method-consumer");
    let relation = MirTypeBridgeDependencyV1::new(
        provider.source.provider,
        MirTypeBridgeTargetV1::Callable(target),
    );
    let dispatch_relation = MirTypeBridgeDependencyV1::new(
        provider.source.provider,
        MirTypeBridgeTargetV1::Dispatch(provider.types.payload.id()),
    );
    consumer.source.uses = vec![relation, dispatch_relation];
    let mut graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    let section = consumer.section(&[&terminal], &graph).unwrap();
    let mut expected = vec![provider.type_use(), relation, dispatch_relation];
    expected.sort_unstable();
    assert_eq!(section.selected().relations().collect::<Vec<_>>(), expected);
    let handle = section
        .selected()
        .reference(relation.provider(), relation.target())
        .unwrap();
    assert!(
        matches!(section.selected().resolve(handle), Some(MirTypeBridgeSemanticRecordV1::Callable(record)) if record.lowered_signature().gc_effect() == crate::GcEffect::NoGc)
    );
    let bytes = encode(&terminal).unwrap();
    let decoded: DecodedCrossConeMirTypeBridgeSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let replayed = decoded
        .validate(
            provider.authority(),
            &[],
            &provider.source,
            &mut graph,
            &mut meter(),
        )
        .unwrap();
    assert_eq!(encode(&replayed).unwrap(), bytes);
}

#[test]
fn a_foundation_signature_cannot_replace_a_missing_callable_export() {
    let mut provider = Fixture::new("private-provider");
    let (function, _) = add_function(&mut provider, "private");
    provider.production = production(provider.source.provider, &provider.types.foundation);
    let mut consumer = Fixture::new("private-consumer");
    let target = MirTypeBridgeTargetV1::Callable(StrongCallableDefinitionOwner::Function(function));
    consumer.source.uses = vec![MirTypeBridgeDependencyV1::new(
        provider.source.provider,
        target,
    )];
    let graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    assert!(
        matches!(consumer.section(&[&terminal], &graph), Err(MirTypeBridgeSectionError::MissingDependency(actual)) if actual == target)
    );
}
