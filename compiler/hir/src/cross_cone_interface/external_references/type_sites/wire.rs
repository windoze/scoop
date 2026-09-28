use super::*;
use scoop_wire::{Decoder, Encoder, WireEncode, WireError, WireErrorKind};

impl WireEncode for HirDependencyTypeSiteV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Expression(site) => {
                encoder.map(6)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                site.position().root.encode(encoder)?;
                encoder.field(2)?;
                encoder.unsigned(u64::from(site.position().expression_index))?;
                encoder.field(3)?;
                site.origin().encode(encoder)?;
                encoder.field(4)?;
                site.role().encode(encoder)?;
                encoder.field(5)?;
                site.exact().encode(encoder)
            }
            Self::CallableSignature {
                root,
                position,
                exact,
            } => {
                encoder.map(4)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                root.encode(encoder)?;
                encoder.field(2)?;
                position.encode(encoder)?;
                encoder.field(3)?;
                exact.encode(encoder)
            }
            Self::LocalValue { local, exact } => declaration(encoder, 3, local, exact),
            Self::BackingStorage { property, exact } => declaration(encoder, 4, property, exact),
            Self::DelegateStorage { property, exact } => declaration(encoder, 5, property, exact),
            Self::FieldStorage {
                owner,
                field,
                exact,
            } => storage(encoder, 6, field, exact, owner),
            Self::EnumVariantFieldStorage {
                owner,
                field,
                exact,
            } => storage(encoder, 7, field, exact, owner),
            Self::ConstructorInitializerResult { constructor, exact } => {
                declaration(encoder, 8, constructor, exact)
            }
            Self::InitializationCycleMessage { unit, exact } => {
                declaration(encoder, 9, unit, exact)
            }
        }
    }
}

pub(super) fn storage(
    encoder: &mut Encoder,
    tag: u64,
    field: &impl WireEncode,
    exact: &impl WireEncode,
    owner: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    field.encode(encoder)?;
    encoder.field(2)?;
    exact.encode(encoder)?;
    encoder.field(3)?;
    owner.encode(encoder)
}

pub(super) fn declaration(
    encoder: &mut Encoder,
    tag: u64,
    owner: &impl WireEncode,
    exact: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    owner.encode(encoder)?;
    encoder.field(2)?;
    exact.encode(encoder)
}

pub(super) fn require_fields(
    decoder: &Decoder<'_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

pub(super) fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}
