use super::*;
use crate::{
    CheckedNominalSupportAccessSourceV1, CheckedNominalSupportCallableSourceV1,
    CheckedProtectedCallableSourceV1, NominalSourceCallablePayloadV1,
    ProtectedDefaultTemplateKeyV1, SourceNominalId,
};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::WireError;

mod coverage;
mod owner_profile;
pub use coverage::*;
pub use owner_profile::*;

/// Definition-side classification of the complete source default, independently
/// of its transport witness. A static nested owner does not capture outer binders.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedDefaultWitnessSourceProfileV1 {
    ParamFree,
    GenericSourceMetadata,
}
pub trait ProtectedDefaultSourceProfileSemanticAuthority<E> {
    fn default_access_profile(
        &self,
        key: ProtectedDefaultTemplateKeyV1,
    ) -> Result<ProtectedDefaultWitnessSourceProfileV1, E>;
}

/// Source inputs can only be obtained by validating complete callable leaves.
#[derive(Clone, Copy, Debug)]
pub enum ProtectedDefaultOwnerSourceV1<'a> {
    Protected(CheckedProtectedCallableSourceV1<'a>),
    NominalSupport(CheckedNominalSupportCallableSourceV1<'a>),
}
impl<'a> ProtectedDefaultOwnerSourceV1<'a> {
    pub const fn declaration(&self) -> CallableTemplateOrigin {
        match self {
            Self::Protected(source) => source.declaration(),
            Self::NominalSupport(source) => source.declaration(),
        }
    }
    pub fn payload(&self) -> &'a NominalSourceCallablePayloadV1 {
        match self {
            Self::Protected(source) => source.payload(),
            Self::NominalSupport(source) => source.payload(),
        }
    }
    pub const fn declaration_access(&self) -> CheckedNominalSupportAccessSourceV1<'a> {
        match self {
            Self::Protected(source) => {
                CheckedNominalSupportAccessSourceV1::Declaration(source.declaration_access())
            }
            Self::NominalSupport(source) => source.declaration_access(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CheckedProtectedDefaultWitnessSourceV1<'w, 's> {
    ParamFree(CheckedParamFreeProtectedDefaultWitnessSourceV1<'w, 's>),
    GenericSourceMetadata(CheckedGenericProtectedDefaultWitnessSourceV1<'w, 's>),
}
/// Source classification only; domain and complete body checks remain required.
#[derive(Clone, Copy, Debug)]
pub struct CheckedParamFreeProtectedDefaultWitnessSourceV1<'w, 's> {
    witness: &'w ParamFreeProtectedDefaultAccessWitnessV1,
    source: ProtectedDefaultOwnerSourceV1<'s>,
}
/// Complete generic owner provenance, without concrete coverage or expansion authority.
#[derive(Clone, Copy, Debug)]
pub struct CheckedGenericProtectedDefaultWitnessSourceV1<'w, 's> {
    witness: &'w ProtectedDefaultAccessWitnessV1,
    source: ProtectedDefaultOwnerSourceV1<'s>,
}
impl CheckedGenericProtectedDefaultWitnessSourceV1<'_, '_> {
    pub const fn witness(&self) -> &ProtectedDefaultAccessWitnessV1 {
        self.witness
    }
    pub const fn source(&self) -> ProtectedDefaultOwnerSourceV1<'_> {
        self.source
    }
}
impl ProtectedDefaultAccessWitnessV1 {
    pub fn validate_source_profile<
        'w,
        's,
        A: ProtectedDefaultSourceProfileSemanticAuthority<E>,
        E,
    >(
        &'w self,
        key: ProtectedDefaultTemplateKeyV1,
        source: ProtectedDefaultOwnerSourceV1<'s>,
        authority: &A,
    ) -> Result<CheckedProtectedDefaultWitnessSourceV1<'w, 's>, ProtectedDefaultWitnessSourceError<E>>
    {
        if self.owner() != key.owner() || self.owner() != source.declaration() {
            return Err(ProtectedDefaultWitnessSourceError::Owner);
        }
        source
            .validate_default_profile(key, authority)?
            .validate_witness(self)
            .map_err(|error| match error {
                ProtectedDefaultWitnessProfileError::Owner => {
                    ProtectedDefaultWitnessSourceError::Owner
                }
                ProtectedDefaultWitnessProfileError::SourceProfile => {
                    ProtectedDefaultWitnessSourceError::SourceProfile
                }
            })
    }
}
#[derive(Debug, Eq, PartialEq)]
pub enum ProtectedDefaultWitnessSourceError<E> {
    Foundation(E),
    Resource(WireError),
    Owner,
    SourceProfile,
}
impl<E: std::fmt::Display> std::fmt::Display for ProtectedDefaultWitnessSourceError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Foundation(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
            Self::Owner => {
                f.write_str("default access witness has a different template/source owner")
            }
            Self::SourceProfile => f.write_str(
                "default access witness branch disagrees with the complete lexical owner chain",
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedDefaultWitnessSourceError<E> {}
