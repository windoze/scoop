use super::wire;
use crate::{
    CanonicalBinderListV1, CanonicalPersistentIdsV1, CanonicalSignatureTypesV1,
    NominalInheritanceModalityV1, NominalSourceShapeV1, PublicNominalKindV1,
};
use scoop_identity::PersistentConstructorId;
use scoop_wire::{Encoder, WireEncode};

mod closure;
mod decode;
mod errors;
mod payload;
mod references;
mod semantics;
mod support;
#[cfg(test)]
mod tests;

pub use decode::*;
pub use errors::*;
pub use payload::*;
pub use references::*;
pub use semantics::*;
pub use support::*;

/// Complete source metadata for a nominal reached through a restricted owner.
/// It does not carry an export binding or grant ordinary public lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedNestedSourceInterfaceV1 {
    kind: PublicNominalKindV1,
    modality: NominalInheritanceModalityV1,
    type_parameters: CanonicalBinderListV1,
    supertypes: CanonicalSignatureTypesV1,
    constructors: CanonicalPersistentIdsV1<PersistentConstructorId>,
    members: CanonicalNestedMemberRefsV1,
    children: CanonicalNestedNominalRefsV1,
    source_shape: NominalSourceShapeV1,
    source_support: CanonicalNestedSourceSupportV1,
}
impl ProtectedNestedSourceInterfaceV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        kind: PublicNominalKindV1,
        modality: NominalInheritanceModalityV1,
        type_parameters: CanonicalBinderListV1,
        supertypes: CanonicalSignatureTypesV1,
        constructors: CanonicalPersistentIdsV1<PersistentConstructorId>,
        members: CanonicalNestedMemberRefsV1,
        children: CanonicalNestedNominalRefsV1,
        source_shape: NominalSourceShapeV1,
        source_support: CanonicalNestedSourceSupportV1,
    ) -> Result<Self, NestedSourceBuildError> {
        if source_shape.kind() != kind
            || (!constructors.is_empty()
                && !matches!(
                    kind,
                    PublicNominalKindV1::Class | PublicNominalKindV1::Struct
                ))
        {
            return Err(NestedSourceBuildError::Kind);
        }
        let valid = match kind {
            PublicNominalKindV1::Class => modality != NominalInheritanceModalityV1::Interface,
            PublicNominalKindV1::Interface => modality == NominalInheritanceModalityV1::Interface,
            PublicNominalKindV1::Struct
            | PublicNominalKindV1::Enum
            | PublicNominalKindV1::Object => modality == NominalInheritanceModalityV1::Final,
        };
        if !valid {
            return Err(NestedSourceBuildError::Modality);
        }
        if kind == PublicNominalKindV1::Object && !type_parameters.is_empty() {
            return Err(NestedSourceBuildError::Binders);
        }
        Ok(Self {
            kind,
            modality,
            type_parameters,
            supertypes,
            constructors,
            members,
            children,
            source_shape,
            source_support,
        })
    }
    pub const fn kind(&self) -> PublicNominalKindV1 {
        self.kind
    }
    pub const fn modality(&self) -> NominalInheritanceModalityV1 {
        self.modality
    }
    pub const fn type_parameters(&self) -> &CanonicalBinderListV1 {
        &self.type_parameters
    }
    pub const fn supertypes(&self) -> &CanonicalSignatureTypesV1 {
        &self.supertypes
    }
    pub const fn constructors(&self) -> &CanonicalPersistentIdsV1<PersistentConstructorId> {
        &self.constructors
    }
    pub const fn members(&self) -> &CanonicalNestedMemberRefsV1 {
        &self.members
    }
    pub const fn children(&self) -> &CanonicalNestedNominalRefsV1 {
        &self.children
    }
    pub const fn source_shape(&self) -> &NominalSourceShapeV1 {
        &self.source_shape
    }
    pub const fn source_support(&self) -> &CanonicalNestedSourceSupportV1 {
        &self.source_support
    }
}
impl WireEncode for ProtectedNestedSourceInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.kind.encode(encoder)?;
        encoder.field(2)?;
        self.modality.encode(encoder)?;
        encoder.field(3)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(4)?;
        self.supertypes.encode(encoder)?;
        encoder.field(5)?;
        self.constructors.encode(encoder)?;
        encoder.field(6)?;
        self.members.encode(encoder)?;
        encoder.field(7)?;
        self.children.encode(encoder)?;
        encoder.field(8)?;
        self.source_shape.encode(encoder)?;
        encoder.field(9)?;
        self.source_support.encode(encoder)
    }
}
