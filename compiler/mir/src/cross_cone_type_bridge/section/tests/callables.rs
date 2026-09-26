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
                fixture.provider,
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
    fixture.production = production(fixture.provider, &fixture.types.foundation);
    let signature = MirBridgeCallableSignatureV1::new(signature, crate::GcEffect::NoGc);
    let method = ParamFreeMirCallableBindingV1::try_new(
        MirCallableBridgeAuthority {
            identities: &fixture.types.graph,
            foundation: &fixture.types.foundation,
            types: fixture.exports.types(),
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
        types: fixture.exports.types(),
        callables: &callables,
    };
    let dispatch = ParamFreeMirDispatchSchemaV1::try_new(
        authority,
        fixture.types.payload.id(),
        MirClassVtableSchemaV1::NoClassVtable,
        vec![],
    )
    .unwrap();
    let dispatch = CanonicalMirDispatchSchemasV1::try_new(authority, vec![dispatch]).unwrap();
    let previous = &fixture.exports;
    fixture.exports = MirTypeBridgeExportConstituentsV1::new(
        previous.types().clone(),
        callables,
        dispatch,
        previous.objects().clone(),
        previous.shapes().clone(),
        previous.initialization_uses().clone(),
    );
    target
}

#[test]
fn value_method_selection_closes_both_signatures_and_preserves_gc_effect() {
    let mut provider = Fixture::new("method-provider");
    let target = value_method(&mut provider);
    let mut consumer = Fixture::new("method-consumer");
    let relation =
        MirTypeBridgeDependencyV1::new(provider.provider, MirTypeBridgeTargetV1::Callable(target));
    let dispatch_relation = MirTypeBridgeDependencyV1::new(
        provider.provider,
        MirTypeBridgeTargetV1::Dispatch(provider.types.payload.id()),
    );
    consumer.uses = vec![relation, dispatch_relation];
    let mut graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    let section = consumer
        .section(&[terminal.dependency_view()], &graph)
        .unwrap();
    let mut expected = vec![provider.type_use(), relation, dispatch_relation];
    expected.sort_unstable();
    assert_eq!(section.selected().relations().collect::<Vec<_>>(), expected);
    assert!(
        matches!(section.selected().record(relation.provider(), relation.target()), Some(MirTypeBridgeSemanticRecordV1::Callable(record)) if record.lowered_signature().gc_effect() == crate::GcEffect::NoGc)
    );
    let bytes = encode(&terminal).unwrap();
    let decoded: DecodedCrossConeMirTypeBridgeSectionV1 = decode_canonical(&bytes).unwrap();
    let replayed = resolved_dependencies::read(&provider, decoded, &[], &mut graph).unwrap();
    assert_eq!(replayed.exports().types(), provider.exports.types());
}

#[test]
fn a_foundation_signature_cannot_replace_a_missing_callable_export() {
    let mut provider = Fixture::new("private-provider");
    let (function, _) = add_function(&mut provider, "private");
    provider.production = production(provider.provider, &provider.types.foundation);
    let mut consumer = Fixture::new("private-consumer");
    let target = MirTypeBridgeTargetV1::Callable(StrongCallableDefinitionOwner::Function(function));
    consumer.uses = vec![MirTypeBridgeDependencyV1::new(provider.provider, target)];
    let graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    assert!(
        matches!(consumer.section(&[terminal.dependency_view()], &graph), Err(MirTypeBridgeSectionError::MissingDependency(actual)) if actual == target)
    );
}
