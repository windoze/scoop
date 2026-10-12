//! Storage identities for interface implementation delegation.

use super::*;

impl FieldIdentityKey {
    pub fn source_interface_delegate(
        owner: &SourceDeclarationKey,
        ordinal: u32,
    ) -> Result<Self, FieldIdentityError> {
        require_source_kind(owner, SourceDeclarationKind::Class)?;
        Ok(Self(FieldIdentityKeyKind::Source(SourceFieldKey(
            SourceFieldKeyKind::InterfaceDelegate {
                owner: source_nominal_owner(owner)?,
                ordinal,
            },
        ))))
    }

    pub fn object_interface_delegate(
        owner: &GeneratedNominalKey,
        ordinal: u32,
    ) -> Result<Self, FieldIdentityError> {
        generated_field(
            owner,
            GeneratedOwnerKind::ObjectBackingClass,
            GeneratedFieldKeyKind::InterfaceDelegate(ordinal),
        )
    }
}

impl GeneratedFieldKey {
    pub const fn interface_delegate(&self) -> Option<u32> {
        match self.0 {
            GeneratedFieldKeyKind::InterfaceDelegate(ordinal) => Some(ordinal),
            _ => None,
        }
    }
}
