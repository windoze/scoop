//! The single declaration-side access snapshot for a default reference.

use scoop_identity::{CallableTemplateOrigin, DecodedCallableTemplateOrigin};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

use crate::{
    CallableDeclarationId, CallableDeclarationIdResolver, DecodedSourceAccessDomainV1,
    SourceAccessDomainResolutionError, SourceAccessDomainResolver, SourceAccessDomainV1,
};

mod errors;
pub use errors::{
    ExportDefaultAccessWitnessBuildError, ExportDefaultAccessWitnessResolutionError,
    PublicDefaultWitnessError,
};

/// The two universal shapes used when validating a public source callable.
/// This query result is not a wire domain or a lookup capability.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExportDefaultCallDomainV1 {
    DirectPublic,
    DirectAndPublicSlot,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExportDefaultAccessWitnessV1 {
    owner: CallableDeclarationId,
    direct: SourceAccessDomainV1,
    slot: Option<SourceAccessDomainV1>,
    target: SourceAccessDomainV1,
}

impl ExportDefaultAccessWitnessV1 {
    pub fn validate_public_access(
        &self,
        owner: CallableDeclarationId,
        domain: ExportDefaultCallDomainV1,
    ) -> Result<(), PublicDefaultWitnessError> {
        if self.owner != owner {
            return Err(PublicDefaultWitnessError::Owner {
                expected: owner,
                actual: self.owner,
            });
        }
        let actual = self.public_call_domain();
        if actual != Some(domain) {
            return Err(PublicDefaultWitnessError::CallDomain {
                expected: domain,
                actual,
            });
        }
        if !self.target.is_universal() {
            return Err(PublicDefaultWitnessError::RestrictedTarget);
        }
        Ok(())
    }

    pub const fn new(owner: CallableDeclarationId, call_domain: ExportDefaultCallDomainV1) -> Self {
        Self {
            owner,
            direct: SourceAccessDomainV1::universal(),
            slot: match call_domain {
                ExportDefaultCallDomainV1::DirectPublic => None,
                ExportDefaultCallDomainV1::DirectAndPublicSlot => {
                    Some(SourceAccessDomainV1::universal())
                }
            },
            target: SourceAccessDomainV1::universal(),
        }
    }

    pub fn try_new(
        owner: CallableDeclarationId,
        direct: SourceAccessDomainV1,
        slot: Option<SourceAccessDomainV1>,
        target: SourceAccessDomainV1,
    ) -> Result<Self, ExportDefaultAccessWitnessBuildError> {
        if matches!(owner, CallableTemplateOrigin::Accessor(_)) {
            return Err(ExportDefaultAccessWitnessBuildError::AccessorOwner);
        }
        if slot.is_some()
            && !matches!(
                owner,
                CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_)
            )
        {
            return Err(ExportDefaultAccessWitnessBuildError::SlotForConstructor);
        }
        Ok(Self {
            owner,
            direct,
            slot,
            target,
        })
    }

    pub const fn owner(&self) -> CallableDeclarationId {
        self.owner
    }
    pub const fn direct_call_domain(&self) -> &SourceAccessDomainV1 {
        &self.direct
    }
    pub const fn slot_call_domain(&self) -> Option<&SourceAccessDomainV1> {
        self.slot.as_ref()
    }
    pub const fn target_domain(&self) -> &SourceAccessDomainV1 {
        &self.target
    }

    pub fn public_call_domain(&self) -> Option<ExportDefaultCallDomainV1> {
        if !self.direct.is_universal() {
            return None;
        }
        match &self.slot {
            None => Some(ExportDefaultCallDomainV1::DirectPublic),
            Some(slot) if slot.is_universal() => {
                Some(ExportDefaultCallDomainV1::DirectAndPublicSlot)
            }
            Some(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExportDefaultAccessWitnessV1 {
    owner: DecodedCallableTemplateOrigin,
    direct: DecodedSourceAccessDomainV1,
    slot: Option<DecodedSourceAccessDomainV1>,
    target: DecodedSourceAccessDomainV1,
}

impl DecodedExportDefaultAccessWitnessV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,

        path: &WirePath,
    ) -> Result<ExportDefaultAccessWitnessV1, ExportDefaultAccessWitnessResolutionError<E>>
    where
        R: CallableDeclarationIdResolver<E> + SourceAccessDomainResolver<E>,
    {
        let owner = self
            .owner
            .resolve(resolver)
            .map_err(ExportDefaultAccessWitnessResolutionError::Owner)?;
        let direct = self.direct.resolve(resolver, &path.clone().field(2))?;
        let slot = self
            .slot
            .map(|slot| slot.resolve(resolver, &path.clone().field(3)))
            .transpose()?;
        let target = self.target.resolve(resolver, &path.clone().field(4))?;
        ExportDefaultAccessWitnessV1::try_new(owner, direct, slot, target)
            .map_err(ExportDefaultAccessWitnessResolutionError::Build)
    }
}

macro_rules! encode_witness {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                e.map(4)?;
                e.field(1)?;
                self.owner.encode(e)?;
                e.field(2)?;
                self.direct.encode(e)?;
                e.field(3)?;
                e.array(u64::from(self.slot.is_some()))?;
                if let Some(slot) = &self.slot {
                    slot.encode(e)?;
                }
                e.field(4)?;
                self.target.encode(e)
            }
        }
    };
}
encode_witness!(ExportDefaultAccessWitnessV1);
encode_witness!(DecodedExportDefaultAccessWitnessV1);

impl WireDecode for DecodedExportDefaultAccessWitnessV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(4)?;
        Ok(Self {
            owner: d.field(1, DecodedCallableTemplateOrigin::decode)?,
            direct: d.field(2, DecodedSourceAccessDomainV1::decode)?,
            slot: d.field(3, |d| match d.array()? {
                0 => Ok(None),
                1 => d.index(0, DecodedSourceAccessDomainV1::decode).map(Some),
                actual => Err(WireError::new(
                    scoop_wire::WireErrorKind::InvalidLength {
                        expected: 1,
                        actual,
                    },
                    d.path().clone(),
                    Some(d.position()),
                )),
            })?,
            target: d.field(4, DecodedSourceAccessDomainV1::decode)?,
        })
    }
}
