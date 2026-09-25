use super::*;

type Nominal = CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>;

pub(super) fn sig(
    receiver: Option<PersistentExactTypeId>,
    parameters: Vec<PersistentExactTypeId>,
    result: PersistentExactTypeId,
) -> ExactCallableSignature {
    ExactCallableSignature::new(Effect::Ordinary, receiver, parameters, result)
}
pub(super) fn signature(exact: ExactCallableSignature) -> MirBridgeCallableSignatureV1 {
    MirBridgeCallableSignatureV1::new(exact, crate::GcEffect::Managed)
}
pub(super) fn generated(record: &crate::GeneratedCallableRecord) -> MirCallableOriginV1 {
    MirCallableOriginV1::Generated {
        callable: record.id(),
        role: record.key().clone(),
    }
}

pub(super) fn name(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
pub(super) fn site(owner: Option<PersistentTypeId>) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(
            owner.into_iter().map(DefinitionOwnerAtom::Type).collect(),
        ),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
pub(super) fn nominal(value: &str, kind: SourceNominalKind) -> Nominal {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(None),
        name(value),
        kind,
        0,
    ))
    .unwrap()
}
pub(super) fn exact_record(
    nominal: PersistentTypeId,
) -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap()
}
pub(super) fn exact(nominal: PersistentTypeId) -> PersistentExactTypeId {
    exact_record(nominal).id()
}
pub(super) fn facts(kind: MirValueKindV1) -> MirTypeFactsV1 {
    MirTypeFactsV1::try_new(
        kind,
        if kind == MirValueKindV1::Reference {
            MirGcKindV1::ContainsManagedReferences
        } else {
            MirGcKindV1::GcFree
        },
    )
    .unwrap()
}
pub(super) fn no_bases() -> MirBaseAndInterfacesV1 {
    MirBaseAndInterfacesV1 {
        base: MirBaseClassV1::None,
        interfaces: vec![],
    }
}
