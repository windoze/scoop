use std::fmt;

use scoop_identity::{
    DecodedPersistentId, ExactTypeKey, PersistentExactTypeId, PersistentTypeId,
    SourceDeclarationKind,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    CanonicalHirFoundation, DefinedCoreProtocols, ExportHir, HirNominalIdentity, Type,
    ValidatedHirFoundation,
};

/// Runtime-visible core capabilities whose semantic owner is established in
/// target-independent Export HIR. Target layout is attached by LIR later.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCoreCapabilityV1 {
    String {
        source_type: PersistentTypeId,
        exact_type: PersistentExactTypeId,
    },
}

impl RuntimeCoreCapabilityV1 {
    pub(super) fn string_from_export(
        export: &ExportHir,
        protocols: &DefinedCoreProtocols,
    ) -> Result<Self, RuntimeCoreCapabilityBuildError> {
        let class = protocols.fundamental_types.string;
        let HirNominalIdentity::Source(source_identity) = &export.nominal_identities[class] else {
            return Err(RuntimeCoreCapabilityBuildError::StringSourceNotSource);
        };
        let source_type = source_identity
            .concrete_id()
            .ok_or(RuntimeCoreCapabilityBuildError::StringSourceIsGeneric)?;
        validate_string_declaration(source_identity.declaration())
            .map_err(|_| RuntimeCoreCapabilityBuildError::InvalidStringDeclaration)?;
        if !matches!(&export.types[export.string], Type::String) {
            return Err(RuntimeCoreCapabilityBuildError::InvalidStringTypeSlot);
        }
        let exact_record = export
            .type_identities
            .get(export.string)
            .and_then(|identity| identity.exact())
            .ok_or(RuntimeCoreCapabilityBuildError::StringTypeIsOpen)?;
        if exact_record.key() != &ExactTypeKey::Nominal(source_type) {
            return Err(RuntimeCoreCapabilityBuildError::StringExactTypeMismatch);
        }
        Ok(Self::String {
            source_type,
            exact_type: exact_record.id(),
        })
    }

    pub const fn source_type(self) -> PersistentTypeId {
        match self {
            Self::String { source_type, .. } => source_type,
        }
    }

    pub const fn exact_type(self) -> PersistentExactTypeId {
        match self {
            Self::String { exact_type, .. } => exact_type,
        }
    }
}

impl WireEncode for RuntimeCoreCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::String {
                source_type,
                exact_type,
            } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                source_type.encode(encoder)?;
                encoder.field(2)?;
                exact_type.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedRuntimeCoreCapabilityV1 {
    String {
        source_type: DecodedPersistentId<PersistentTypeId>,
        exact_type: DecodedPersistentId<PersistentExactTypeId>,
    },
}

impl DecodedRuntimeCoreCapabilityV1 {
    pub fn validate(
        self,
        foundation: &ValidatedHirFoundation,
    ) -> Result<RuntimeCoreCapabilityV1, RuntimeCoreCapabilityValidationError> {
        self.validate_against(foundation.canonical())
    }

    pub(super) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<RuntimeCoreCapabilityV1, RuntimeCoreCapabilityValidationError> {
        match self {
            Self::String {
                source_type,
                exact_type,
            } => {
                let (source_type, declaration) = foundation
                    .source_type_by_bytes(source_type.as_array())
                    .ok_or(RuntimeCoreCapabilityValidationError::UnknownStringSource(
                        *source_type.as_array(),
                    ))?;
                validate_string_declaration(declaration).map_err(|_| {
                    RuntimeCoreCapabilityValidationError::InvalidStringDeclaration(source_type)
                })?;
                let (exact_type, exact_key) = foundation
                    .exact_type_by_bytes(exact_type.as_array())
                    .ok_or(RuntimeCoreCapabilityValidationError::UnknownStringExact(
                        *exact_type.as_array(),
                    ))?;
                if exact_key != &ExactTypeKey::Nominal(source_type) {
                    return Err(
                        RuntimeCoreCapabilityValidationError::StringExactTypeMismatch {
                            source_type,
                            exact_type,
                        },
                    );
                }
                Ok(RuntimeCoreCapabilityV1::String {
                    source_type,
                    exact_type,
                })
            }
        }
    }
}

impl WireEncode for DecodedRuntimeCoreCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::String {
                source_type,
                exact_type,
            } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                source_type.encode(encoder)?;
                encoder.field(2)?;
                exact_type.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedRuntimeCoreCapabilityV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, fields, 3)?;
                Ok(Self::String {
                    source_type: decoder.field(1, DecodedPersistentId::decode)?,
                    exact_type: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCoreCapabilityBuildError {
    StringSourceNotSource,
    StringSourceIsGeneric,
    InvalidStringDeclaration,
    InvalidStringTypeSlot,
    StringTypeIsOpen,
    StringExactTypeMismatch,
}

impl fmt::Display for RuntimeCoreCapabilityBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StringSourceNotSource => {
                formatter.write_str("core String intrinsic owner is not a source declaration")
            }
            Self::StringSourceIsGeneric => {
                formatter.write_str("core String intrinsic owner is generic")
            }
            Self::InvalidStringDeclaration => {
                formatter.write_str("core String source declaration has an invalid identity shape")
            }
            Self::InvalidStringTypeSlot => {
                formatter.write_str("core String type slot does not contain the String type")
            }
            Self::StringTypeIsOpen => formatter.write_str("core String type is open"),
            Self::StringExactTypeMismatch => formatter
                .write_str("core String exact type does not name its intrinsic source declaration"),
        }
    }
}

impl std::error::Error for RuntimeCoreCapabilityBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCoreCapabilityValidationError {
    UnknownStringSource([u8; 32]),
    UnknownStringExact([u8; 32]),
    InvalidStringDeclaration(PersistentTypeId),
    StringExactTypeMismatch {
        source_type: PersistentTypeId,
        exact_type: PersistentExactTypeId,
    },
}

impl fmt::Display for RuntimeCoreCapabilityValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownStringSource(id) => write!(
                formatter,
                "runtime String source type {} is absent from the HIR foundation",
                Hex(id)
            ),
            Self::UnknownStringExact(id) => write!(
                formatter,
                "runtime String exact type {} is absent from the HIR foundation",
                Hex(id)
            ),
            Self::InvalidStringDeclaration(id) => write!(
                formatter,
                "runtime String source type {id} is not a non-generic class declaration"
            ),
            Self::StringExactTypeMismatch {
                source_type,
                exact_type,
            } => write!(
                formatter,
                "runtime String exact type {exact_type} is not the nominal exact identity of {source_type}"
            ),
        }
    }
}

impl std::error::Error for RuntimeCoreCapabilityValidationError {}

fn validate_string_declaration(
    declaration: &scoop_identity::SourceDeclarationKey,
) -> Result<(), ()> {
    let valid = declaration.declaration_kind() == SourceDeclarationKind::Class
        && declaration.duplicate_signature().type_parameter_count() == 0;
    valid.then_some(()).ok_or(())
}

fn require_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

struct Hex<'a>(&'a [u8; 32]);

impl fmt::Display for Hex<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
