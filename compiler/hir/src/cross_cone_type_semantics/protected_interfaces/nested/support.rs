use super::*;
use crate::{
    DeclarationAccessSourceV1, NominalSupportCallableInterfaceV1,
    NominalSupportConstructorInterfaceV1, NominalSupportPropertyInterfaceV1, SourceNominalId,
};
use scoop_identity::{CallableTemplateOrigin, PersistentConstructorId, PersistentPropertyId};
use scoop_wire::{Encoder, WireEncode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NestedSupportDeclarationV1 {
    Callable(CallableTemplateOrigin),
    Constructor(PersistentConstructorId),
    Property(PersistentPropertyId),
    NestedNominal(SourceNominalId),
}
impl WireEncode for NestedSupportDeclarationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Callable(id) => {
                wire::tag(encoder, 2, 1)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
            Self::Constructor(id) => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
            Self::Property(id) => {
                wire::tag(encoder, 2, 3)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
            Self::NestedNominal(id) => {
                wire::tag(encoder, 2, 4)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NestedSourceSupportV1 {
    Callable(Box<NominalSupportCallableInterfaceV1>),
    Constructor(Box<NominalSupportConstructorInterfaceV1>),
    Property(Box<NominalSupportPropertyInterfaceV1>),
    NestedNominal(Box<NominalSupportNestedInterfaceV1>),
}
impl NestedSourceSupportV1 {
    pub fn declaration(&self) -> NestedSupportDeclarationV1 {
        match self {
            Self::Callable(record) => NestedSupportDeclarationV1::Callable(record.declaration()),
            Self::Constructor(record) => {
                NestedSupportDeclarationV1::Constructor(record.declaration())
            }
            Self::Property(record) => NestedSupportDeclarationV1::Property(record.declaration()),
            Self::NestedNominal(record) => {
                NestedSupportDeclarationV1::NestedNominal(record.declaration())
            }
        }
    }
    pub fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        match self {
            Self::Callable(record) => record.declaration_access(),
            Self::Constructor(record) => record.declaration_access(),
            Self::Property(record) => record.declaration_access(),
            Self::NestedNominal(record) => record.declaration_access(),
        }
    }
}
impl WireEncode for NestedSourceSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Callable(record) => {
                wire::tag(encoder, 4, 1)?;
                encoder.field(1)?;
                record.declaration().encode(encoder)?;
                encoder.field(2)?;
                record.declaration_access().encode(encoder)?;
                encoder.field(3)?;
                record.payload().encode(encoder)
            }
            Self::Constructor(record) => {
                wire::tag(encoder, 4, 2)?;
                encoder.field(1)?;
                record.declaration().encode(encoder)?;
                encoder.field(2)?;
                record.declaration_access().encode(encoder)?;
                encoder.field(3)?;
                record.payload().encode(encoder)
            }
            Self::Property(record) => {
                wire::tag(encoder, 4, 3)?;
                encoder.field(1)?;
                record.declaration().encode(encoder)?;
                encoder.field(2)?;
                record.declaration_access().encode(encoder)?;
                encoder.field(3)?;
                record.payload().encode(encoder)
            }
            Self::NestedNominal(record) => {
                wire::tag(encoder, 4, 4)?;
                encoder.field(1)?;
                record.declaration().encode(encoder)?;
                encoder.field(2)?;
                record.declaration_access().encode(encoder)?;
                encoder.field(3)?;
                record.payload().encode(encoder)
            }
        }
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalNestedSourceSupportV1 {
    records: Vec<NestedSourceSupportV1>,
}
impl CanonicalNestedSourceSupportV1 {
    pub fn try_new(records: Vec<NestedSourceSupportV1>) -> Result<Self, NestedSourceBuildError> {
        let mut keyed = records
            .into_iter()
            .map(|record| {
                Ok((
                    scoop_wire::encode(&record.declaration())
                        .map_err(NestedSourceBuildError::Encoding)?,
                    record,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        keyed.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        Self::from_ordered(keyed.into_iter().map(|(_, record)| record).collect())
    }
    pub(super) fn from_ordered(
        records: Vec<NestedSourceSupportV1>,
    ) -> Result<Self, NestedSourceBuildError> {
        references::validate_order(
            &records
                .iter()
                .map(NestedSourceSupportV1::declaration)
                .collect::<Vec<_>>(),
        )?;
        Ok(Self { records })
    }
    pub fn records(&self) -> &[NestedSourceSupportV1] {
        &self.records
    }
}
impl WireEncode for CanonicalNestedSourceSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
