use super::{CanonicalProtectedSlotRefsV1, wire};
use crate::{
    DeclarationAccessSourceV1, DeclaredVisibilityV1, PropertyRepresentationV1, SourceNominalId,
};
use scoop_identity::{PersistentPropertyAccessorId, PersistentPropertyId, SignatureTypeKey};
use scoop_wire::{Encoder, WireEncode};

mod decode;
mod errors;
pub(super) mod semantics;
mod source_decode;
mod source_payload;
#[cfg(test)]
pub(super) mod tests;

pub use decode::*;
pub use errors::*;
pub use semantics::*;
pub use source_decode::*;
pub use source_payload::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtectedPropertyMutabilityV1 {
    ReadOnly,
    ReadWrite {
        setter: PersistentPropertyAccessorId,
        setter_access: DeclarationAccessSourceV1,
    },
}
impl WireEncode for ProtectedPropertyMutabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ReadOnly => wire::tag(encoder, 1, 1),
            Self::ReadWrite {
                setter,
                setter_access,
            } => {
                wire::tag(encoder, 3, 2)?;
                encoder.field(1)?;
                setter.encode(encoder)?;
                encoder.field(2)?;
                setter_access.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedPropertyPayloadV1 {
    pub(in crate::cross_cone_type_semantics::protected_interfaces) source:
        NominalSourcePropertyPayloadV1,
}
impl ProtectedPropertyPayloadV1 {
    pub fn try_new(
        owner: SourceNominalId,
        value_type: SignatureTypeKey,
        getter: PersistentPropertyAccessorId,
        mutability: ProtectedPropertyMutabilityV1,
        representation: PropertyRepresentationV1,
        slot_relations: CanonicalProtectedSlotRefsV1,
    ) -> Result<Self, ProtectedPropertyBuildError> {
        Self::from_source_payload(NominalSourcePropertyPayloadV1::try_new(
            owner,
            value_type,
            getter,
            mutability,
            representation,
            slot_relations,
        )?)
    }
    pub(super) fn from_source_payload(
        source: NominalSourcePropertyPayloadV1,
    ) -> Result<Self, ProtectedPropertyBuildError> {
        if matches!(source.mutability(), ProtectedPropertyMutabilityV1::ReadWrite { setter_access, .. } if setter_access.declared_visibility() == DeclaredVisibilityV1::Public)
        {
            return Err(ProtectedPropertyBuildError::SetterAccess);
        }
        Ok(Self { source })
    }
}
impl std::ops::Deref for ProtectedPropertyPayloadV1 {
    type Target = NominalSourcePropertyPayloadV1;
    fn deref(&self) -> &Self::Target {
        &self.source
    }
}
impl WireEncode for ProtectedPropertyPayloadV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.source.encode(encoder)
    }
}

/// A protected logical property constituent. Accessor and slot closure are
/// checked separately before the complete section grants selection authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedPropertyInterfaceV1 {
    declaration: PersistentPropertyId,
    declaration_access: DeclarationAccessSourceV1,
    payload: ProtectedPropertyPayloadV1,
}
impl ProtectedPropertyInterfaceV1 {
    pub fn try_new(
        declaration: PersistentPropertyId,
        declaration_access: DeclarationAccessSourceV1,
        payload: ProtectedPropertyPayloadV1,
    ) -> Result<Self, ProtectedPropertyBuildError> {
        use ProtectedPropertyBuildError as Error;
        if declaration_access.declared_visibility() != DeclaredVisibilityV1::Protected {
            return Err(Error::Access);
        }
        if declaration_access.lexical_owners().last().copied() != Some(payload.owner()) {
            return Err(Error::Owner);
        }
        if let ProtectedPropertyMutabilityV1::ReadWrite { setter_access, .. } = payload.mutability()
        {
            if setter_access.lexical_owners() != declaration_access.lexical_owners()
                || setter_access.definition_origin().origin().source()
                    != declaration_access.definition_origin().origin().source()
            {
                return Err(Error::SetterAccess);
            }
        }
        Ok(Self {
            declaration,
            declaration_access,
            payload,
        })
    }
    pub const fn declaration(&self) -> PersistentPropertyId {
        self.declaration
    }
    pub const fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        &self.declaration_access
    }
    pub const fn payload(&self) -> &ProtectedPropertyPayloadV1 {
        &self.payload
    }
}
impl WireEncode for ProtectedPropertyInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.declaration_access.encode(encoder)?;
        encoder.field(3)?;
        self.payload.encode(encoder)
    }
}
