use super::{ProtectedCallableInterfaceBuildError as Error, ProtectedCallablePayloadV1};
use crate::{DeclarationAccessSourceV1, DeclaredVisibilityV1};
use scoop_identity::{CallableTemplateOrigin, PersistentConstructorId};
use scoop_wire::{Encoder, WireEncode};

/// Source-interface constituent, not a public lookup or protected access proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedCallableInterfaceV1 {
    declaration: CallableTemplateOrigin,
    declaration_access: DeclarationAccessSourceV1,
    payload: ProtectedCallablePayloadV1,
}
impl ProtectedCallableInterfaceV1 {
    pub fn try_new(
        declaration: CallableTemplateOrigin,
        declaration_access: DeclarationAccessSourceV1,
        payload: ProtectedCallablePayloadV1,
    ) -> Result<Self, Error> {
        if !matches!(
            declaration,
            CallableTemplateOrigin::Function(_)
                | CallableTemplateOrigin::GenericFunction(_)
                | CallableTemplateOrigin::Accessor(_)
        ) {
            return Err(Error::DeclarationKind);
        }
        validate_access(&declaration_access, &payload)?;
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
    pub const fn payload(&self) -> &ProtectedCallablePayloadV1 {
        &self.payload
    }
}
impl WireEncode for ProtectedCallableInterfaceV1 {
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
pub struct ProtectedConstructorInterfaceV1 {
    declaration: PersistentConstructorId,
    declaration_access: DeclarationAccessSourceV1,
    payload: ProtectedCallablePayloadV1,
}
impl ProtectedConstructorInterfaceV1 {
    pub fn try_new(
        declaration: PersistentConstructorId,
        declaration_access: DeclarationAccessSourceV1,
        payload: ProtectedCallablePayloadV1,
    ) -> Result<Self, Error> {
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
    pub const fn payload(&self) -> &ProtectedCallablePayloadV1 {
        &self.payload
    }
}
impl WireEncode for ProtectedConstructorInterfaceV1 {
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
    payload: &ProtectedCallablePayloadV1,
) -> Result<(), Error> {
    if access.declared_visibility() != DeclaredVisibilityV1::Protected {
        return Err(Error::Access);
    }
    if access.lexical_owners().last().copied() != Some(payload.owner()) {
        return Err(Error::Owner);
    }
    Ok(())
}
