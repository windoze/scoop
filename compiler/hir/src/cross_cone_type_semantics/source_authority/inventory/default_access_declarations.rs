use super::*;
use crate::DeclarationAccessSourceV1;
use scoop_identity::DefinitionOriginSubject;
use scoop_wire::{Encoder, WireEncode};

mod wire;
pub use wire::*;

/// Independent declaration visibility and origin. This is not a checked access domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultSourceAccessDeclarationV1 {
    subject: DefinitionOriginSubject,
    access: DeclarationAccessSourceV1,
}
impl DefaultSourceAccessDeclarationV1 {
    pub fn try_new(
        subject: DefinitionOriginSubject,
        access: DeclarationAccessSourceV1,
    ) -> Result<Self, SourceInventoryError> {
        Self::validate_subject(subject)?;
        Ok(Self { subject, access })
    }
    pub(crate) fn validate_subject(
        subject: DefinitionOriginSubject,
    ) -> Result<(), SourceInventoryError> {
        use DefinitionOriginSubject::*;
        if matches!(
            subject,
            Type(_)
                | GenericType(_)
                | Function(_)
                | GenericFunction(_)
                | Constructor(_)
                | Property(_)
                | ExtensionProperty(_)
                | PropertyAccessor(_)
        ) {
            Ok(())
        } else {
            Err(SourceInventoryError::InvalidDefaultAccessSubject(subject))
        }
    }
    pub const fn subject(&self) -> DefinitionOriginSubject {
        self.subject
    }
    pub const fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        &self.access
    }
}
impl WireEncode for DefaultSourceAccessDeclarationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.subject.encode(encoder)?;
        encoder.field(2)?;
        self.access.encode(encoder)
    }
}

/// A canonical source transcript, still requiring artifact and demand-closure binding.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalDefaultSourceAccessDeclarationsV1 {
    records: Vec<DefaultSourceAccessDeclarationV1>,
}
impl CanonicalDefaultSourceAccessDeclarationsV1 {
    pub fn try_new(
        mut records: Vec<DefaultSourceAccessDeclarationV1>,
    ) -> Result<Self, SourceInventoryError> {
        records.sort_unstable_by_key(DefaultSourceAccessDeclarationV1::subject);
        Self::from_ordered(records)
    }
    fn from_ordered(
        records: Vec<DefaultSourceAccessDeclarationV1>,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            DefaultSourceAccessDeclarationV1::subject,
            "default access declarations",
        )?;
        Ok(Self { records })
    }
    pub fn records(&self) -> &[DefaultSourceAccessDeclarationV1] {
        &self.records
    }
    pub fn get(
        &self,
        subject: DefinitionOriginSubject,
    ) -> Option<&DefaultSourceAccessDeclarationV1> {
        self.records
            .binary_search_by_key(&subject, DefaultSourceAccessDeclarationV1::subject)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalDefaultSourceAccessDeclarationsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::wire::sequence(encoder, &self.records)
    }
}
