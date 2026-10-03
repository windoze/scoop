use super::*;
use crate::{IndexedExportTemplateFragmentV1, TemplateFragmentIndexError};

pub struct IndexedExportGenericInitializationsV1<'a> {
    records: Vec<IndexedNominalInitialization<'a>>,
}

struct IndexedNominalInitialization<'a> {
    owner: PersistentGenericTypeId,
    common: Vec<IndexedCommonStep<'a>>,
    constructors: Vec<IndexedConstructor<'a>>,
}

enum IndexedCommonStep<'a> {
    Field(&'a DefaultFieldRefV1, IndexedExportTemplateFragmentV1<'a>),
    Body(IndexedExportTemplateFragmentV1<'a>),
}

struct IndexedDelegation<'a> {
    target: &'a DefaultConstructorRefV1,
    arguments: IndexedExportTemplateFragmentV1<'a>,
}

struct IndexedConstructor<'a> {
    source: &'a ExportConstructorInitializationV1,
    kind: IndexedConstructorKind<'a>,
}

enum IndexedConstructorKind<'a> {
    StructPrimary,
    StructSecondary(IndexedDelegation<'a>, IndexedExportTemplateFragmentV1<'a>),
    ClassPrimary(
        Option<IndexedDelegation<'a>>,
        &'a [ExportPrimaryFieldStoreV1],
    ),
    ClassSecondaryThis(IndexedDelegation<'a>, IndexedExportTemplateFragmentV1<'a>),
    ClassSecondaryTerminal(
        Option<IndexedDelegation<'a>>,
        IndexedExportTemplateFragmentV1<'a>,
    ),
}

impl CanonicalExportGenericInitializationsV1 {
    pub fn index_locals(
        &self,
    ) -> Result<IndexedExportGenericInitializationsV1<'_>, TemplateFragmentIndexError> {
        let records = self
            .records
            .iter()
            .map(|record| {
                let common = record
                    .common
                    .iter()
                    .map(|step| {
                        Ok(match step {
                            ExportCommonInitializationStepV1::Field { field, value } => {
                                IndexedCommonStep::Field(field, value.index_locals()?)
                            }
                            ExportCommonInitializationStepV1::Body(body) => {
                                IndexedCommonStep::Body(body.index_locals()?)
                            }
                        })
                    })
                    .collect::<Result<_, TemplateFragmentIndexError>>()?;
                let constructors = record
                    .constructors
                    .iter()
                    .map(|source| {
                        Ok(IndexedConstructor {
                            source,
                            kind: index_kind(&source.kind)?,
                        })
                    })
                    .collect::<Result<_, TemplateFragmentIndexError>>()?;
                Ok(IndexedNominalInitialization {
                    owner: record.owner,
                    common,
                    constructors,
                })
            })
            .collect::<Result<_, TemplateFragmentIndexError>>()?;
        Ok(IndexedExportGenericInitializationsV1 { records })
    }
}

fn index_kind(
    kind: &ExportConstructorInitializationKindV1,
) -> Result<IndexedConstructorKind<'_>, TemplateFragmentIndexError> {
    use ExportConstructorInitializationKindV1 as Kind;
    Ok(match kind {
        Kind::StructPrimary => IndexedConstructorKind::StructPrimary,
        Kind::StructSecondary { delegation, body } => IndexedConstructorKind::StructSecondary(
            index_delegation(delegation)?,
            body.index_locals()?,
        ),
        Kind::ClassPrimary {
            base,
            primary_stores,
        } => IndexedConstructorKind::ClassPrimary(
            base.as_ref().map(index_delegation).transpose()?,
            primary_stores,
        ),
        Kind::ClassSecondaryThis { delegation, body } => {
            IndexedConstructorKind::ClassSecondaryThis(
                index_delegation(delegation)?,
                body.index_locals()?,
            )
        }
        Kind::ClassSecondaryTerminal { base, body } => {
            IndexedConstructorKind::ClassSecondaryTerminal(
                base.as_ref().map(index_delegation).transpose()?,
                body.index_locals()?,
            )
        }
    })
}

fn index_delegation(
    delegation: &ExportConstructorDelegationV1,
) -> Result<IndexedDelegation<'_>, TemplateFragmentIndexError> {
    Ok(IndexedDelegation {
        target: &delegation.target,
        arguments: delegation.arguments.index_locals()?,
    })
}

impl WireEncode for IndexedExportGenericInitializationsV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_values(encoder, &self.records)
    }
}

impl WireEncode for IndexedNominalInitialization<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        encode_values(encoder, &self.common)?;
        encoder.field(3)?;
        encode_values(encoder, &self.constructors)
    }
}

impl WireEncode for IndexedCommonStep<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Field(field, value) => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                field.encode(encoder)?;
                encoder.field(2)?;
                value.encode(encoder)
            }
            Self::Body(body) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                body.encode(encoder)
            }
        }
    }
}

impl WireEncode for IndexedDelegation<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.arguments.encode(encoder)
    }
}

impl WireEncode for IndexedConstructor<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.source.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.source.inputs.encode(encoder)?;
        encoder.field(3)?;
        self.source.effects.encode(encoder)?;
        encoder.field(4)?;
        self.source.predicates.encode(encoder)?;
        encoder.field(5)?;
        self.source.definition_origin.encode(encoder)?;
        encoder.field(6)?;
        self.kind.encode(encoder)
    }
}

impl WireEncode for IndexedConstructorKind<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if matches!(self, Self::StructPrimary) {
            1
        } else {
            3
        })?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::StructPrimary => 1,
            Self::StructSecondary(..) => 2,
            Self::ClassPrimary(..) => 3,
            Self::ClassSecondaryThis(..) => 4,
            Self::ClassSecondaryTerminal(..) => 5,
        })?;
        match self {
            Self::StructPrimary => Ok(()),
            Self::StructSecondary(delegation, body)
            | Self::ClassSecondaryThis(delegation, body) => {
                encoder.field(1)?;
                delegation.encode(encoder)?;
                encoder.field(2)?;
                body.encode(encoder)
            }
            Self::ClassPrimary(base, stores) => {
                encoder.field(1)?;
                encode_base(encoder, base.as_ref())?;
                encoder.field(2)?;
                encoder.array(stores.len() as u64)?;
                for store in *stores {
                    encoder.map(2)?;
                    encoder.field(1)?;
                    store.field.encode(encoder)?;
                    encoder.field(2)?;
                    store.parameter.encode(encoder)?;
                }
                Ok(())
            }
            Self::ClassSecondaryTerminal(base, body) => {
                encoder.field(1)?;
                encode_base(encoder, base.as_ref())?;
                encoder.field(2)?;
                body.encode(encoder)
            }
        }
    }
}

pub(super) fn encode_base<T: WireEncode>(
    encoder: &mut Encoder,
    base: Option<&T>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(u64::from(base.is_some()))?;
    if let Some(base) = base {
        base.encode(encoder)?;
    }
    Ok(())
}
