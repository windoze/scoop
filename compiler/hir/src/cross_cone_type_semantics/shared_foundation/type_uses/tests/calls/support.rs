use super::*;

pub(super) fn dependencies<'a>(core: &'a Loaded, provider: &'a Loaded) -> Vec<&'a Loaded> {
    if core.provider() == provider.provider() {
        vec![provider]
    } else {
        vec![core, provider]
    }
}

pub(super) fn nominal_owner(owner: PersistentTypeId) -> PublicDeclarationOwnerV1 {
    PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner))
}

pub(super) fn representation(
    provider: ConeIdentity,
    owner: PersistentTypeId,
) -> SelectedExternalTypeUseV1 {
    SelectedExternalTypeUseV1::new(
        provider,
        SelectedTypeUseV1::Representation {
            exact: exact(owner),
        },
    )
}

pub(super) fn signature(
    provider: ConeIdentity,
    owner: PersistentTypeId,
) -> SelectedExternalTypeUseV1 {
    SelectedExternalTypeUseV1::new(
        provider,
        SelectedTypeUseV1::Signature {
            exact: exact(owner),
        },
    )
}

pub(super) fn signature_uses(
    provider: ConeIdentity,
    owners: &[PersistentTypeId],
) -> Vec<SelectedExternalTypeUseV1> {
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let mut uses = vec![
        representation(ConeIdentity::CORE, unit),
        signature(ConeIdentity::CORE, unit),
    ];
    for owner in owners {
        uses.push(representation(provider, *owner));
        uses.push(signature(provider, *owner));
    }
    uses
}

pub(super) fn member_call(
    provider: ConeIdentity,
    receiver: PersistentTypeId,
    declaration: InheritanceCallableDeclarationV1,
) -> SelectedExternalTypeUseV1 {
    SelectedExternalTypeUseV1::new(
        provider,
        SelectedTypeUseV1::MemberCall {
            receiver: exact(receiver),
            declaration,
        },
    )
}
