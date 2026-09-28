mod constructors;

use super::*;
use crate::cross_cone_interface::external_references::call_sites::Fixture as CallFixture;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
    DefinitionOwnerAtom, DefinitionOwnerChain, Effect, ExactTypeKey, GcEffect, PackagePath,
    PendingIdentityValidation, PersistentFunctionId, PersistentGenericTypeId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, ValidatedIdentityGraph,
};

pub(super) struct Fixture {
    call: CallFixture,
    identities: ValidatedIdentityGraph,
    foundation: crate::OdrFreeHirFoundation,
    pub public: crate::CrossConeHirInterfaceSectionV1,
    pub target: ExternalHirTargetV1,
    pub receiver: crate::SourceCallReceiver<PersistentExactTypeId>,
}

impl Fixture {
    pub fn new(
        owner: PublicDeclarationOwnerV1,
        receiver: Option<SignatureTypeKey>,
        parameters: Vec<SignatureTypeKey>,
        result: SignatureTypeKey,
        extra_keys: Vec<ExactTypeKey>,
    ) -> Self {
        let call = CallFixture::new();
        let owners = match owner.nominal_owner() {
            Some(SourceNominalId::Concrete(owner)) => vec![DefinitionOwnerAtom::Type(owner)],
            Some(SourceNominalId::GenericTemplate(owner)) => {
                vec![DefinitionOwnerAtom::GenericType(owner)]
            }
            None => Vec::new(),
        };
        let function: CborIdentityRecord<PersistentFunctionId, _> =
            CborIdentityRecord::from_key(SourceDeclarationKey::function(
                site(owners),
                identifier("invoke"),
                0,
                receiver.clone(),
                parameters.clone(),
            ))
            .unwrap();
        let target = CallableTemplateOrigin::Function(function.id());
        let source_receiver = match owner.nominal_owner() {
            Some(SourceNominalId::Concrete(owner)) => crate::SourceCallReceiver::Receiver {
                static_type: exact(ExactTypeKey::Nominal(owner)),
            },
            Some(SourceNominalId::GenericTemplate(_)) => crate::SourceCallReceiver::NoReceiver,
            None => match receiver.as_ref() {
                Some(SignatureTypeKey::Nominal(owner)) => crate::SourceCallReceiver::Receiver {
                    static_type: exact(ExactTypeKey::Nominal(*owner)),
                },
                Some(other) => panic!("fixture requires an explicit concrete receiver: {other:?}"),
                None => crate::SourceCallReceiver::NoReceiver,
            },
        };
        let declaration = crate::CallableDeclarationRecordV1::try_new(
            target,
            owner,
            crate::CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            receiver,
            crate::CanonicalSourceParameterShapesV1::try_new(
                parameters
                    .into_iter()
                    .enumerate()
                    .map(|(index, ty)| {
                        crate::SourceParameterShapeV1::new(identifier(&format!("p{index}")), ty)
                    })
                    .collect(),
            )
            .unwrap(),
            result,
            crate::CallableSourceEffectsV1::try_new(
                Effect::Ordinary,
                crate::CallableSafetyV1::Safe,
                GcEffect::Managed,
                crate::CallableImplementationV1::Scoop,
                crate::CallableOperatorRoleV1::None,
                crate::CallableInfixV1::Ordinary,
            )
            .unwrap(),
            crate::CallableModalityV1::Final,
            crate::DeclaredVisibilityV1::Public,
            crate::CanonicalPersistentIdsV1::empty(),
        )
        .unwrap();
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        pending.register_authority(call.provider).unwrap();
        pending
            .register_external_canonical_authority(function)
            .unwrap();
        pending
            .register_external_canonical_authority(CoreBuiltinNominal::Unit.identity_record())
            .unwrap();
        pending
            .register_external_canonical_authority(flag())
            .unwrap();
        pending
            .register_external_canonical_authority(nominal())
            .unwrap();
        pending
            .register_external_canonical_authority(generic_nominal())
            .unwrap();
        for key in [
            ExactTypeKey::Nominal(unit_owner()),
            ExactTypeKey::Nominal(bool_owner()),
            ExactTypeKey::Nominal(nominal().id()),
        ]
        .into_iter()
        .chain(extra_keys)
        {
            pending
                .register_external_canonical_authority(
                    CborIdentityRecord::<PersistentExactTypeId, _>::from_key(key).unwrap(),
                )
                .unwrap();
        }
        Self {
            call,
            identities: pending.finish().unwrap(),
            foundation:
                crate::OdrFreeHirFoundation::try_new(crate::CanonicalHirFoundation::empty())
                    .unwrap(),
            public: crate::CrossConeHirInterfaceSectionV1::new(
                Default::default(),
                Default::default(),
                crate::CanonicalCallableInterfacesV1::with_support(Vec::new(), vec![declaration])
                    .unwrap(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            ),
            target: ExternalHirTargetV1::Callable(target),
            receiver: source_receiver,
        }
    }

    pub fn simple() -> Self {
        Self::new(
            PublicDeclarationOwnerV1::TopLevel,
            None,
            vec![unit(), unit()],
            unit(),
            Vec::new(),
        )
    }

    pub fn metadata(&self) -> SharedTypeMetadataV1<'_> {
        SharedTypeMetadataV1 {
            provider: self.call.provider,
            identities: &self.identities,
            foundation: &self.foundation,
            public: &self.public,
        }
    }

    pub fn call(
        &self,
        index: u32,
        arguments: Vec<PersistentExactTypeId>,
        result: PersistentExactTypeId,
    ) -> HirDependencyCallSiteV1 {
        let site = self.call.site(index, vec![0]).unwrap();
        HirDependencyCallSiteV1::try_new(
            site.position(),
            site.origin().clone(),
            arguments,
            result,
            vec![0],
            self.receiver,
        )
        .unwrap()
    }
}

fn site(owners: Vec<DefinitionOwnerAtom>) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        CallFixture::new().provider,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(owners),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(name: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(name).unwrap()
}
pub(super) fn unit_owner() -> PersistentTypeId {
    CoreBuiltinNominal::Unit.identity_record().id()
}
pub(super) fn bool_owner() -> PersistentTypeId {
    flag().id()
}
fn flag() -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(Vec::new()),
        identifier("Flag"),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
}
pub(super) fn unit() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(unit_owner())
}
pub(super) fn boolean() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(bool_owner())
}
pub(super) fn exact(key: ExactTypeKey) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&key).unwrap()
}
pub(super) fn unit_exact() -> PersistentExactTypeId {
    exact(ExactTypeKey::Nominal(unit_owner()))
}
pub(super) fn bool_exact() -> PersistentExactTypeId {
    exact(ExactTypeKey::Nominal(bool_owner()))
}

pub(super) fn nominal() -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(Vec::new()),
        identifier("Owner"),
        SourceNominalKind::Class,
        0,
    ))
    .unwrap()
}

pub(super) fn generic_nominal() -> CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey>
{
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(Vec::new()),
        identifier("GenericOwner"),
        SourceNominalKind::Class,
        1,
    ))
    .unwrap()
}
