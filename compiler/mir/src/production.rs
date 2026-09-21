//! Canonical MIR bridge metadata for the single-Cone strong profile.

use std::fmt;

use scoop_identity::{
    CallableOwner, CborIdentityRecord, CoreBuiltinNominal, DecodedCallableOwner,
    DecodedExactCallableSignature, DecodedExecutableSourceEntryIdentity, ExactCallableSignature,
    ExactCallableSignatureResolutionError, ExactOrdinaryNoArgUnitSignature, ExactTypeKey,
    ExecutableSourceEntryIdentity, ExecutableSourceEntryIdentityError, IdentityReferenceError,
    PersistentExactTypeId, PersistentFunctionId, PersistentKeyResolver, SourceDeclarationKey,
    ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

use crate::{
    CallableSignatureSubject, CanonicalMirFoundation, OdrFreeMirFoundation, ValidatedMirFoundation,
};

/// Fixed exact identity of core `Unit`, shared by bridge projections without
/// exposing identity-construction details to a lowering implementation.
pub fn core_unit_exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .expect("the fixed core Unit exact-type identity is hashable")
}

mod callables;
use callables::DecodedStrongCallableBridgeSurfaceV1;
pub use callables::{StrongCallableBridgeSurfaceV1, StrongCallableBridgeV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryMirBridgeV1 {
    source: ExecutableSourceEntryIdentity,
    implementation: CallableOwner,
}

impl EntryMirBridgeV1 {
    pub fn new(
        source: ExecutableSourceEntryIdentity,
        implementation: CallableOwner,
    ) -> Result<Self, MirProductionBuildError> {
        let source_entry = source.declaration();
        if implementation != CallableOwner::Function(source_entry) {
            return Err(MirProductionBuildError::EntryImplementationMismatch {
                source_entry,
                implementation,
            });
        }
        Ok(Self {
            source,
            implementation,
        })
    }

    pub const fn source(&self) -> &ExecutableSourceEntryIdentity {
        &self.source
    }

    pub const fn implementation(&self) -> CallableOwner {
        self.implementation
    }
}

impl WireEncode for EntryMirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.source.encode(encoder)?;
        encoder.field(2)?;
        self.implementation.encode(encoder)
    }
}

#[derive(Debug)]
struct DecodedEntryMirBridgeV1 {
    source: DecodedExecutableSourceEntryIdentity,
    implementation: DecodedCallableOwner,
}

impl WireEncode for DecodedEntryMirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.source.encode(encoder)?;
        encoder.field(2)?;
        self.implementation.encode(encoder)
    }
}

impl WireDecode for DecodedEntryMirBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            source: decoder.field(1, DecodedExecutableSourceEntryIdentity::decode)?,
            implementation: decoder.field(2, DecodedCallableOwner::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryMirBridgeBranchV1 {
    Library,
    Executable(Box<EntryMirBridgeV1>),
}

impl WireEncode for EntryMirBridgeBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Library => encode_empty_sum(encoder, 1),
            Self::Executable(bridge) => encode_value_sum(encoder, 2, bridge.as_ref()),
        }
    }
}

#[derive(Debug)]
enum DecodedEntryMirBridgeBranchV1 {
    Library,
    Executable(Box<DecodedEntryMirBridgeV1>),
}

impl WireEncode for DecodedEntryMirBridgeBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Library => encode_empty_sum(encoder, 1),
            Self::Executable(bridge) => encode_value_sum(encoder, 2, bridge.as_ref()),
        }
    }
}

impl WireDecode for DecodedEntryMirBridgeBranchV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decode_sum(decoder, |tag, decoder| match tag {
            1 => Ok(Self::Library),
            2 => DecodedEntryMirBridgeV1::decode(decoder)
                .map(Box::new)
                .map(Self::Executable),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreBootstrapBridgeSectionV1 {
    entry_bridge: EntryMirBridgeBranchV1,
    strong_callable_bridges: StrongCallableBridgeSurfaceV1,
}

impl CoreBootstrapBridgeSectionV1 {
    pub fn try_new(
        artifact: scoop_identity::ConeIdentity,
        entry_bridge: EntryMirBridgeBranchV1,
        strong_callable_bridges: StrongCallableBridgeSurfaceV1,
    ) -> Result<Self, MirProductionBuildError> {
        validate_section_relations(artifact, &entry_bridge, &strong_callable_bridges)?;
        Ok(Self {
            entry_bridge,
            strong_callable_bridges,
        })
    }

    pub(crate) fn validate_for_artifact(
        &self,
        artifact: scoop_identity::ConeIdentity,
    ) -> Result<(), MirProductionBuildError> {
        validate_section_relations(artifact, &self.entry_bridge, &self.strong_callable_bridges)
    }

    pub const fn entry_bridge(&self) -> &EntryMirBridgeBranchV1 {
        &self.entry_bridge
    }

    pub const fn strong_callable_bridges(&self) -> &StrongCallableBridgeSurfaceV1 {
        &self.strong_callable_bridges
    }
}

impl WireEncode for CoreBootstrapBridgeSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(2)?;
        self.entry_bridge.encode(encoder)?;
        encoder.field(3)?;
        self.strong_callable_bridges.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedCoreBootstrapBridgeSectionV1 {
    entry_bridge: DecodedEntryMirBridgeBranchV1,
    strong_callable_bridges: DecodedStrongCallableBridgeSurfaceV1,
}

impl DecodedCoreBootstrapBridgeSectionV1 {
    pub fn validate(
        self,
        artifact: scoop_identity::ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
        foundation: &ValidatedMirFoundation,
    ) -> Result<CoreBootstrapBridgeSectionV1, MirProductionValidationError> {
        self.validate_against(artifact, identities, foundation.canonical())
    }

    pub fn validate_against_strong_foundation(
        self,
        artifact: scoop_identity::ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
        foundation: &OdrFreeMirFoundation,
    ) -> Result<CoreBootstrapBridgeSectionV1, MirProductionValidationError> {
        self.validate_against(artifact, identities, foundation.as_canonical())
    }

    fn validate_against(
        self,
        artifact: scoop_identity::ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
        foundation: &CanonicalMirFoundation,
    ) -> Result<CoreBootstrapBridgeSectionV1, MirProductionValidationError> {
        let strong_callable_bridges = self
            .strong_callable_bridges
            .validate(identities, foundation)?;
        let entry_bridge = match self.entry_bridge {
            DecodedEntryMirBridgeBranchV1::Library => EntryMirBridgeBranchV1::Library,
            DecodedEntryMirBridgeBranchV1::Executable(bridge) => {
                let source = resolve_entry_source(bridge.source, artifact, identities)?;
                let source_entry = source.declaration();
                let implementation = bridge
                    .implementation
                    .resolve(identities)
                    .map_err(MirProductionValidationError::Identity)?;
                if implementation != CallableOwner::Function(source_entry) {
                    return Err(MirProductionValidationError::EntryImplementationMismatch {
                        source_entry,
                        implementation,
                    });
                }
                EntryMirBridgeBranchV1::Executable(Box::new(EntryMirBridgeV1 {
                    source,
                    implementation,
                }))
            }
        };
        validate_section_relations(artifact, &entry_bridge, &strong_callable_bridges)
            .map_err(MirProductionValidationError::Relation)?;
        Ok(CoreBootstrapBridgeSectionV1 {
            entry_bridge,
            strong_callable_bridges,
        })
    }
}

impl WireEncode for DecodedCoreBootstrapBridgeSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(2)?;
        self.entry_bridge.encode(encoder)?;
        encoder.field(3)?;
        self.strong_callable_bridges.encode(encoder)
    }
}

impl WireDecode for DecodedCoreBootstrapBridgeSectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            entry_bridge: decoder.field(2, DecodedEntryMirBridgeBranchV1::decode)?,
            strong_callable_bridges: decoder
                .field(3, DecodedStrongCallableBridgeSurfaceV1::decode)?,
        })
    }
}

fn resolve_entry_source(
    decoded: DecodedExecutableSourceEntryIdentity,
    artifact: scoop_identity::ConeIdentity,
    identities: &mut ValidatedIdentityGraph,
) -> Result<ExecutableSourceEntryIdentity, MirProductionValidationError> {
    let declaration_key: std::sync::Arc<SourceDeclarationKey> = identities
        .resolve_key(decoded.declaration())
        .map_err(MirProductionValidationError::Identity)?;
    let declaration = CborIdentityRecord::from_key((*declaration_key).clone())
        .map_err(MirProductionValidationError::EntrySourceRecord)?;
    let unit = CborIdentityRecord::<PersistentExactTypeId, ExactTypeKey>::from_key(
        ExactTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
    )
    .map_err(MirProductionValidationError::EntryUnitRecord)?
    .id();
    let expected = ExecutableSourceEntryIdentity::try_new(
        &declaration,
        ExactOrdinaryNoArgUnitSignature::new(unit),
    )
    .map_err(MirProductionValidationError::InvalidEntrySource)?;
    if expected.root_cone() != artifact {
        return Err(MirProductionValidationError::ForeignEntrySource {
            artifact,
            entry: expected.root_cone(),
        });
    }
    let actual_bytes = encode(&decoded).map_err(MirProductionValidationError::EntrySourceEncode)?;
    let expected_bytes =
        encode(&expected).map_err(MirProductionValidationError::EntrySourceEncode)?;
    if actual_bytes != expected_bytes {
        return Err(MirProductionValidationError::EntrySourceMismatch);
    }
    Ok(expected)
}

fn validate_section_relations(
    artifact: scoop_identity::ConeIdentity,
    entry_bridge: &EntryMirBridgeBranchV1,
    strong_callable_bridges: &StrongCallableBridgeSurfaceV1,
) -> Result<(), MirProductionBuildError> {
    if let EntryMirBridgeBranchV1::Executable(entry) = entry_bridge
        && entry.source().root_cone() != artifact
    {
        return Err(MirProductionBuildError::ForeignEntrySource {
            artifact,
            entry: entry.source().root_cone(),
        });
    }
    match (
        artifact == scoop_identity::ConeIdentity::CORE,
        strong_callable_bridges.initialization_cycle().is_some(),
    ) {
        (true, false) => return Err(MirProductionBuildError::MissingInitializationCycle),
        (false, true) => {
            return Err(MirProductionBuildError::UnexpectedInitializationCycle(
                artifact,
            ));
        }
        (true, true) | (false, false) => {}
    }
    if artifact == scoop_identity::ConeIdentity::CORE
        && entry_bridge != &EntryMirBridgeBranchV1::Library
    {
        return Err(MirProductionBuildError::CoreMustBeLibrary);
    }
    if let EntryMirBridgeBranchV1::Executable(entry) = entry_bridge
        && strong_callable_bridges.get(entry.implementation).is_none()
    {
        return Err(MirProductionBuildError::MissingStrongEntryImplementation(
            entry.implementation,
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirProductionBuildError {
    DuplicateStrongCallable(CallableOwner),
    EntryImplementationMismatch {
        source_entry: PersistentFunctionId,
        implementation: CallableOwner,
    },
    ForeignEntrySource {
        artifact: scoop_identity::ConeIdentity,
        entry: scoop_identity::ConeIdentity,
    },
    MissingInitializationCycle,
    UnexpectedInitializationCycle(scoop_identity::ConeIdentity),
    DuplicateInitializationCycle,
    InvalidInitializationCycleOwner(CallableOwner),
    CoreMustBeLibrary,
    MissingStrongInitializationCycle(CallableOwner),
    MissingStrongEntryImplementation(CallableOwner),
}

impl fmt::Display for MirProductionBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot build MIR production bridge: {self:?}")
    }
}

impl std::error::Error for MirProductionBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum MirProductionValidationError {
    Identity(IdentityReferenceError),
    Signature(ExactCallableSignatureResolutionError<IdentityReferenceError>),
    EntrySourceRecord(scoop_identity::SourceDeclarationIdentityError),
    EntryUnitRecord(scoop_wire::HashError),
    InvalidEntrySource(ExecutableSourceEntryIdentityError),
    ForeignEntrySource {
        artifact: scoop_identity::ConeIdentity,
        entry: scoop_identity::ConeIdentity,
    },
    EntrySourceEncode(scoop_wire::cbor::EncodeError),
    EntrySourceMismatch,
    StrongCallableCoverage {
        expected: usize,
        actual: usize,
    },
    FoundationOdrSubject,
    DuplicateStrongCallable(CallableOwner),
    NonCanonicalStrongCallableOrder {
        index: usize,
    },
    StrongCallableMismatch {
        index: usize,
    },
    EntryImplementationMismatch {
        source_entry: PersistentFunctionId,
        implementation: CallableOwner,
    },
    Relation(MirProductionBuildError),
}

impl fmt::Display for MirProductionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid MIR production bridge: {self:?}")
    }
}

impl std::error::Error for MirProductionValidationError {}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn decode_sum<T>(
    decoder: &mut Decoder<'_, '_>,
    decode_value: impl FnOnce(u64, &mut Decoder<'_, '_>) -> Result<T, WireError>,
) -> Result<T, WireError> {
    let fields = decoder.map()?;
    if fields == 0 {
        return Err(wire_error(
            decoder,
            WireErrorKind::MissingField { field: 0 },
        ));
    }
    let tag = decoder.field(0, Decoder::unsigned)?;
    if !matches!(tag, 1 | 2) {
        return Err(wire_error(decoder, WireErrorKind::UnknownTag { tag }));
    }
    let expected = if tag == 1 { 1 } else { 2 };
    if fields != expected {
        return Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength {
                expected,
                actual: fields,
            },
        ));
    }
    if expected == 1 {
        decode_value(tag, decoder)
    } else {
        decoder.field(1, |decoder| decode_value(tag, decoder))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
