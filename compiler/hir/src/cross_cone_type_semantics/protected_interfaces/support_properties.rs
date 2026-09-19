use super::{NominalSourcePropertyPayloadV1, ProtectedPropertyMutabilityV1, wire};
use crate::{DeclarationAccessSourceV1, ExportConstValueV1, SourceNominalId};
use scoop_identity::PersistentPropertyId;
use scoop_wire::{Encoder, WireEncode};

mod decode;
mod errors;
mod semantics;
#[cfg(test)]
mod tests;
pub use decode::*;
pub use errors::*;
pub use semantics::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalSupportPropertyPayloadV1 {
    Runtime {
        interface: NominalSourcePropertyPayloadV1,
    },
    Const {
        value: ExportConstValueV1,
    },
}
impl WireEncode for NominalSupportPropertyPayloadV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Runtime { interface } => {
                wire::tag(encoder, 2, 1)?;
                encoder.field(1)?;
                interface.encode(encoder)
            }
            Self::Const { value } => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                value.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalSupportPropertyInterfaceV1 {
    declaration: PersistentPropertyId,
    declaration_access: DeclarationAccessSourceV1,
    payload: NominalSupportPropertyPayloadV1,
    owner: SourceNominalId,
}
impl NominalSupportPropertyInterfaceV1 {
    pub fn try_new(
        declaration: PersistentPropertyId,
        declaration_access: DeclarationAccessSourceV1,
        payload: NominalSupportPropertyPayloadV1,
    ) -> Result<Self, NominalSupportPropertyBuildError> {
        use NominalSupportPropertyBuildError as Error;
        let owner = declaration_access
            .lexical_owners()
            .last()
            .copied()
            .ok_or(Error::Owner)?;
        match &payload {
            NominalSupportPropertyPayloadV1::Runtime { interface } => {
                if interface.owner() != owner {
                    return Err(Error::Owner);
                }
                if let ProtectedPropertyMutabilityV1::ReadWrite { setter_access, .. } =
                    interface.mutability()
                {
                    if setter_access.lexical_owners() != declaration_access.lexical_owners()
                        || setter_access.definition_origin().origin().source()
                            != declaration_access.definition_origin().origin().source()
                    {
                        return Err(Error::Setter);
                    }
                }
            }
            NominalSupportPropertyPayloadV1::Const { value } => {
                if value.property() != declaration {
                    return Err(Error::ConstIdentity);
                }
                if value.definition_origin() != declaration_access.definition_origin() {
                    return Err(Error::ConstOrigin);
                }
            }
        }
        Ok(Self {
            declaration,
            declaration_access,
            payload,
            owner,
        })
    }
    pub const fn declaration(&self) -> PersistentPropertyId {
        self.declaration
    }
    pub const fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        &self.declaration_access
    }
    pub const fn payload(&self) -> &NominalSupportPropertyPayloadV1 {
        &self.payload
    }
    pub const fn owner(&self) -> SourceNominalId {
        self.owner
    }
}
impl WireEncode for NominalSupportPropertyInterfaceV1 {
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
