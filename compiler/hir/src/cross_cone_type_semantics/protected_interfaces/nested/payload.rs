use super::*;
use crate::{DeclarationAccessSourceV1, DeclaredVisibilityV1, SourceNominalId};
use scoop_identity::{PersistentExactTypeId, PersistentTypeId};
use scoop_wire::{Encoder, WireEncode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NestedNominalSupportV1 {
    ParamFree {
        inheritance_exact: PersistentExactTypeId,
        representation_owner: PersistentTypeId,
    },
    GenericTemplate,
}
impl WireEncode for NestedNominalSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ParamFree {
                inheritance_exact,
                representation_owner,
            } => {
                wire::tag(encoder, 3, 1)?;
                encoder.field(1)?;
                inheritance_exact.encode(encoder)?;
                encoder.field(2)?;
                representation_owner.encode(encoder)
            }
            Self::GenericTemplate => wire::tag(encoder, 1, 2),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedNestedNominalPayloadV1 {
    source_nominal: SourceNominalId,
    source_interface: ProtectedNestedSourceInterfaceV1,
    support: NestedNominalSupportV1,
}
impl ProtectedNestedNominalPayloadV1 {
    pub fn try_new(
        source_nominal: SourceNominalId,
        source_interface: ProtectedNestedSourceInterfaceV1,
        support: NestedNominalSupportV1,
    ) -> Result<Self, NestedSourceBuildError> {
        match (source_nominal, support) {
            (
                SourceNominalId::Concrete(id),
                NestedNominalSupportV1::ParamFree {
                    representation_owner,
                    ..
                },
            ) if id == representation_owner && source_interface.type_parameters().is_empty() => {}
            (SourceNominalId::GenericTemplate(_), NestedNominalSupportV1::GenericTemplate)
                if !source_interface.type_parameters().is_empty() => {}
            _ => return Err(NestedSourceBuildError::SupportKind),
        }
        source_interface.validate_reference_closure(source_nominal)?;
        Ok(Self {
            source_nominal,
            source_interface,
            support,
        })
    }
    pub const fn source_nominal(&self) -> SourceNominalId {
        self.source_nominal
    }
    pub const fn source_interface(&self) -> &ProtectedNestedSourceInterfaceV1 {
        &self.source_interface
    }
    pub const fn support(&self) -> NestedNominalSupportV1 {
        self.support
    }
}
impl WireEncode for ProtectedNestedNominalPayloadV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.source_nominal.encode(encoder)?;
        encoder.field(2)?;
        self.source_interface.encode(encoder)?;
        encoder.field(3)?;
        self.support.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalSupportNestedInterfaceV1 {
    declaration: SourceNominalId,
    declaration_access: DeclarationAccessSourceV1,
    payload: ProtectedNestedNominalPayloadV1,
}
impl NominalSupportNestedInterfaceV1 {
    pub fn try_new(
        declaration: SourceNominalId,
        declaration_access: DeclarationAccessSourceV1,
        payload: ProtectedNestedNominalPayloadV1,
    ) -> Result<Self, NestedSourceBuildError> {
        if declaration != payload.source_nominal() || declaration_access.lexical_owners().is_empty()
        {
            return Err(NestedSourceBuildError::Owner);
        }
        Ok(Self {
            declaration,
            declaration_access,
            payload,
        })
    }
    pub const fn declaration(&self) -> SourceNominalId {
        self.declaration
    }
    pub const fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        &self.declaration_access
    }
    pub const fn payload(&self) -> &ProtectedNestedNominalPayloadV1 {
        &self.payload
    }
}
impl WireEncode for NominalSupportNestedInterfaceV1 {
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
pub struct ProtectedNestedNominalInterfaceV1 {
    pub(super) source: NominalSupportNestedInterfaceV1,
}
impl TryFrom<NominalSupportNestedInterfaceV1> for ProtectedNestedNominalInterfaceV1 {
    type Error = NestedSourceBuildError;

    fn try_from(source: NominalSupportNestedInterfaceV1) -> Result<Self, Self::Error> {
        Self::try_new(
            source.declaration,
            source.declaration_access,
            source.payload,
        )
    }
}
impl ProtectedNestedNominalInterfaceV1 {
    pub(in crate::cross_cone_type_semantics) const fn source_record(
        &self,
    ) -> &NominalSupportNestedInterfaceV1 {
        &self.source
    }
    pub fn try_new(
        declaration: SourceNominalId,
        declaration_access: DeclarationAccessSourceV1,
        payload: ProtectedNestedNominalPayloadV1,
    ) -> Result<Self, NestedSourceBuildError> {
        if declaration_access.declared_visibility() != DeclaredVisibilityV1::Protected {
            return Err(NestedSourceBuildError::Access);
        }
        Ok(Self {
            source: NominalSupportNestedInterfaceV1::try_new(
                declaration,
                declaration_access,
                payload,
            )?,
        })
    }
    pub const fn declaration(&self) -> SourceNominalId {
        self.source.declaration()
    }
    pub const fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        self.source.declaration_access()
    }
    pub const fn payload(&self) -> &ProtectedNestedNominalPayloadV1 {
        self.source.payload()
    }
}
impl WireEncode for ProtectedNestedNominalInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.source.encode(encoder)
    }
}
