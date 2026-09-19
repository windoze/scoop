use super::{NominalSourceCallablePayloadV1, ProtectedCallableInterfaceBuildError as BuildError};
use crate::{CallableModalityV1, DeclarationAccessSourceV1, DeclaredVisibilityV1};
use scoop_identity::{CallableTemplateOrigin, PersistentConstructorId};
use scoop_wire::{Encoder, WireEncode};

mod decode;
mod payload_decode;
mod semantics;
#[cfg(test)]
mod tests;
mod variants;
pub use decode::*;
pub use payload_decode::*;
pub use semantics::*;
pub use variants::*;

/// Source support is stored under an already restricted nominal. Its declared
/// visibility is preserved without creating any ordinary lookup capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalSupportCallableInterfaceV1 {
    declaration: CallableTemplateOrigin,
    declaration_access: DeclarationAccessSourceV1,
    payload: NominalSourceCallablePayloadV1,
}
impl NominalSupportCallableInterfaceV1 {
    pub fn try_new(
        declaration: CallableTemplateOrigin,
        declaration_access: DeclarationAccessSourceV1,
        payload: NominalSourceCallablePayloadV1,
    ) -> Result<Self, BuildError> {
        if !matches!(
            declaration,
            CallableTemplateOrigin::Function(_)
                | CallableTemplateOrigin::GenericFunction(_)
                | CallableTemplateOrigin::Accessor(_)
                | CallableTemplateOrigin::VariantConstructor(_)
        ) {
            return Err(BuildError::DeclarationKind);
        }
        validate_access(&declaration_access, &payload)?;
        if matches!(declaration, CallableTemplateOrigin::VariantConstructor(_))
            && declaration_access.declared_visibility() != DeclaredVisibilityV1::Public
        {
            return Err(BuildError::Access);
        }

        payload.validate_declaration(declaration)?;
        Ok(Self {
            declaration,
            declaration_access,
            payload,
        })
    }
    pub const fn declaration(&self) -> CallableTemplateOrigin {
        self.declaration
    }
    pub const fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        &self.declaration_access
    }
    pub const fn payload(&self) -> &NominalSourceCallablePayloadV1 {
        &self.payload
    }
}
impl WireEncode for NominalSupportCallableInterfaceV1 {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalSupportConstructorInterfaceV1 {
    declaration: PersistentConstructorId,
    declaration_access: DeclarationAccessSourceV1,
    payload: NominalSourceCallablePayloadV1,
}
impl NominalSupportConstructorInterfaceV1 {
    pub fn try_new(
        declaration: PersistentConstructorId,
        declaration_access: DeclarationAccessSourceV1,
        payload: NominalSourceCallablePayloadV1,
    ) -> Result<Self, BuildError> {
        validate_access(&declaration_access, &payload)?;
        payload.validate_declaration(CallableTemplateOrigin::Constructor(declaration))?;
        Ok(Self {
            declaration,
            declaration_access,
            payload,
        })
    }
    pub const fn declaration(&self) -> PersistentConstructorId {
        self.declaration
    }
    pub const fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        &self.declaration_access
    }
    pub const fn payload(&self) -> &NominalSourceCallablePayloadV1 {
        &self.payload
    }
}
impl WireEncode for NominalSupportConstructorInterfaceV1 {
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
fn validate_access(
    access: &DeclarationAccessSourceV1,
    payload: &NominalSourceCallablePayloadV1,
) -> Result<(), BuildError> {
    if access.lexical_owners().last().copied() != Some(payload.owner()) {
        return Err(BuildError::Owner);
    }
    if access.declared_visibility() == DeclaredVisibilityV1::Private
        && (payload.modality() != CallableModalityV1::Final || !payload.slot_relations().is_empty())
    {
        return Err(BuildError::Modality);
    }
    Ok(())
}
