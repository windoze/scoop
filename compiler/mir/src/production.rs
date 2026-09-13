//! Canonical MIR bridge metadata for the single-Cone strong profile.

use std::fmt;

use scoop_identity::{
    CallableOwner, CborIdentityRecord, CoreBuiltinNominal, DecodedCallableOwner,
    DecodedExactCallableSignature, DecodedExecutableSourceEntryIdentity, DecodedPersistentId,
    ExactCallableSignature, ExactCallableSignatureResolutionError, ExactOrdinaryNoArgUnitSignature,
    ExactTypeKey, ExecutableSourceEntryIdentity, ExecutableSourceEntryIdentityError,
    IdentityReferenceError, PersistentExactTypeId, PersistentExportBindingId, PersistentFunctionId,
    PersistentIdResolver, PersistentKeyResolver, SourceDeclarationKey, ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

use crate::{
    CallableSignatureSubject, CanonicalMirFoundation, OdrFreeMirFoundation, ValidatedMirFoundation,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongCallableBridgeV1 {
    implementation: CallableOwner,
    signature: ExactCallableSignature,
}

impl StrongCallableBridgeV1 {
    pub const fn new(implementation: CallableOwner, signature: ExactCallableSignature) -> Self {
        Self {
            implementation,
            signature,
        }
    }

    pub const fn implementation(&self) -> CallableOwner {
        self.implementation
    }

    pub const fn subject(&self) -> CallableSignatureSubject {
        CallableSignatureSubject::Strong(self.implementation)
    }

    pub const fn signature(&self) -> &ExactCallableSignature {
        &self.signature
    }
}

impl WireEncode for StrongCallableBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.implementation.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)
    }
}

#[derive(Debug)]
struct DecodedStrongCallableBridgeV1 {
    implementation: DecodedCallableOwner,
    signature: DecodedExactCallableSignature,
}

impl WireEncode for DecodedStrongCallableBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.implementation.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)
    }
}

impl WireDecode for DecodedStrongCallableBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            implementation: decoder.field(1, DecodedCallableOwner::decode)?,
            signature: decoder.field(2, DecodedExactCallableSignature::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongCallableBridgeSurfaceV1 {
    bridges: Vec<StrongCallableBridgeV1>,
}

impl StrongCallableBridgeSurfaceV1 {
    pub fn from_odr_free_foundation(foundation: &OdrFreeMirFoundation) -> Self {
        let bridges = foundation
            .as_canonical()
            .callable_signatures()
            .iter()
            .map(|record| {
                let CallableSignatureSubject::Strong(implementation) = record.subject() else {
                    unreachable!("OdrFreeMirFoundation excludes ODR signature subjects")
                };
                StrongCallableBridgeV1::new(implementation, record.signature().clone())
            })
            .collect();
        Self { bridges }
    }

    pub fn try_new(
        mut bridges: Vec<StrongCallableBridgeV1>,
    ) -> Result<Self, MirProductionBuildError> {
        bridges.sort_unstable_by_key(StrongCallableBridgeV1::implementation);
        if let Some(pair) = bridges
            .windows(2)
            .find(|pair| pair[0].implementation == pair[1].implementation)
        {
            return Err(MirProductionBuildError::DuplicateStrongCallable(
                pair[0].implementation,
            ));
        }
        Ok(Self { bridges })
    }

    pub fn bridges(&self) -> &[StrongCallableBridgeV1] {
        &self.bridges
    }

    fn get(&self, implementation: CallableOwner) -> Option<&StrongCallableBridgeV1> {
        self.bridges
            .binary_search_by_key(&implementation, StrongCallableBridgeV1::implementation)
            .ok()
            .map(|index| &self.bridges[index])
    }
}

impl WireEncode for StrongCallableBridgeSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.bridges.len() as u64)?;
        for bridge in &self.bridges {
            bridge.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct DecodedStrongCallableBridgeSurfaceV1 {
    bridges: Vec<DecodedStrongCallableBridgeV1>,
}

impl DecodedStrongCallableBridgeSurfaceV1 {
    fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
        foundation: &CanonicalMirFoundation,
    ) -> Result<StrongCallableBridgeSurfaceV1, MirProductionValidationError> {
        if self.bridges.len() != foundation.callable_signatures().len() {
            return Err(MirProductionValidationError::StrongCallableCoverage {
                expected: foundation.callable_signatures().len(),
                actual: self.bridges.len(),
            });
        }
        let mut bridges: Vec<StrongCallableBridgeV1> = Vec::with_capacity(self.bridges.len());
        for (index, decoded) in self.bridges.into_iter().enumerate() {
            let bridge = resolve_strong_bridge(decoded, identities)?;
            if index > 0 && bridges[index - 1].implementation >= bridge.implementation {
                return Err(
                    if bridges[index - 1].implementation == bridge.implementation {
                        MirProductionValidationError::DuplicateStrongCallable(bridge.implementation)
                    } else {
                        MirProductionValidationError::NonCanonicalStrongCallableOrder { index }
                    },
                );
            }
            bridges.push(bridge);
        }
        for (index, (bridge, expected)) in bridges
            .iter()
            .zip(foundation.callable_signatures())
            .enumerate()
        {
            let CallableSignatureSubject::Strong(expected_implementation) = expected.subject()
            else {
                return Err(MirProductionValidationError::FoundationOdrSubject);
            };
            if bridge.implementation != expected_implementation
                || bridge.signature != *expected.signature()
            {
                return Err(MirProductionValidationError::StrongCallableMismatch { index });
            }
        }
        Ok(StrongCallableBridgeSurfaceV1 { bridges })
    }
}

impl WireEncode for DecodedStrongCallableBridgeSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.bridges.len() as u64)?;
        for bridge in &self.bridges {
            bridge.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedStrongCallableBridgeSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedStrongCallableBridgeV1::decode(decoder))
            .map(|bridges| Self { bridges })
    }
}

fn resolve_strong_bridge(
    decoded: DecodedStrongCallableBridgeV1,
    identities: &mut ValidatedIdentityGraph,
) -> Result<StrongCallableBridgeV1, MirProductionValidationError> {
    let implementation = decoded
        .implementation
        .resolve(identities)
        .map_err(MirProductionValidationError::Identity)?;
    let signature = decoded
        .signature
        .resolve(identities)
        .map_err(MirProductionValidationError::Signature)?;
    Ok(StrongCallableBridgeV1 {
        implementation,
        signature,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreMirCallableBridgeV1 {
    binding: PersistentExportBindingId,
    definition: PersistentFunctionId,
    implementation: CallableOwner,
}

impl CoreMirCallableBridgeV1 {
    pub fn new(
        binding: PersistentExportBindingId,
        definition: PersistentFunctionId,
        implementation: CallableOwner,
    ) -> Result<Self, MirProductionBuildError> {
        if implementation != CallableOwner::Function(definition) {
            return Err(MirProductionBuildError::CoreImplementationMismatch {
                definition,
                implementation,
            });
        }
        Ok(Self {
            binding,
            definition,
            implementation,
        })
    }

    pub const fn binding(self) -> PersistentExportBindingId {
        self.binding
    }

    pub const fn definition(self) -> PersistentFunctionId {
        self.definition
    }

    pub const fn implementation(self) -> CallableOwner {
        self.implementation
    }
}

impl WireEncode for CoreMirCallableBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.binding.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        self.implementation.encode(encoder)
    }
}

#[derive(Debug)]
struct DecodedCoreMirCallableBridgeV1 {
    binding: DecodedPersistentId<PersistentExportBindingId>,
    definition: DecodedPersistentId<PersistentFunctionId>,
    implementation: DecodedCallableOwner,
}

impl WireEncode for DecodedCoreMirCallableBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.binding.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        self.implementation.encode(encoder)
    }
}

impl WireDecode for DecodedCoreMirCallableBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            binding: decoder.field(1, DecodedPersistentId::decode)?,
            definition: decoder.field(2, DecodedPersistentId::decode)?,
            implementation: decoder.field(3, DecodedCallableOwner::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreMirBridgeV1 {
    callable_targets: Vec<CoreMirCallableBridgeV1>,
}

impl CoreMirBridgeV1 {
    pub fn try_new(
        mut callable_targets: Vec<CoreMirCallableBridgeV1>,
    ) -> Result<Self, MirProductionBuildError> {
        callable_targets.sort_unstable_by_key(|bridge| bridge.binding);
        if let Some(pair) = callable_targets
            .windows(2)
            .find(|pair| pair[0].binding == pair[1].binding)
        {
            return Err(MirProductionBuildError::DuplicateCoreBinding(
                pair[0].binding,
            ));
        }
        if let Some((index, bridge)) =
            callable_targets.iter().enumerate().find(|(index, bridge)| {
                callable_targets[..*index]
                    .iter()
                    .any(|existing| existing.implementation == bridge.implementation)
            })
        {
            return Err(MirProductionBuildError::DuplicateCoreImplementation {
                index,
                implementation: bridge.implementation,
            });
        }
        Ok(Self { callable_targets })
    }

    pub fn callable_targets(&self) -> &[CoreMirCallableBridgeV1] {
        &self.callable_targets
    }
}

impl WireEncode for CoreMirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        encoder.array(self.callable_targets.len() as u64)?;
        for bridge in &self.callable_targets {
            bridge.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct DecodedCoreMirBridgeV1 {
    callable_targets: Vec<DecodedCoreMirCallableBridgeV1>,
}

impl DecodedCoreMirBridgeV1 {
    fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CoreMirBridgeV1, MirProductionValidationError> {
        let mut callable_targets: Vec<CoreMirCallableBridgeV1> =
            Vec::with_capacity(self.callable_targets.len());
        for (index, decoded) in self.callable_targets.into_iter().enumerate() {
            let binding = identities
                .resolve(decoded.binding)
                .map_err(MirProductionValidationError::Identity)?;
            let definition = identities
                .resolve(decoded.definition)
                .map_err(MirProductionValidationError::Identity)?;
            let implementation = decoded
                .implementation
                .resolve(identities)
                .map_err(MirProductionValidationError::Identity)?;
            if implementation != CallableOwner::Function(definition) {
                return Err(MirProductionValidationError::CoreImplementationMismatch {
                    definition,
                    implementation,
                });
            }
            if index > 0 && callable_targets[index - 1].binding >= binding {
                return Err(if callable_targets[index - 1].binding == binding {
                    MirProductionValidationError::DuplicateCoreBinding(binding)
                } else {
                    MirProductionValidationError::NonCanonicalCoreBindingOrder { index }
                });
            }
            if callable_targets[..index]
                .iter()
                .any(|existing| existing.implementation == implementation)
            {
                return Err(MirProductionValidationError::DuplicateCoreImplementation {
                    index,
                    implementation,
                });
            }
            callable_targets.push(CoreMirCallableBridgeV1 {
                binding,
                definition,
                implementation,
            });
        }
        Ok(CoreMirBridgeV1 { callable_targets })
    }
}

impl WireEncode for DecodedCoreMirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        encoder.array(self.callable_targets.len() as u64)?;
        for bridge in &self.callable_targets {
            bridge.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCoreMirBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        Ok(Self {
            callable_targets: decoder.field(1, |decoder| {
                decoder.decode_array(|decoder, _| DecodedCoreMirCallableBridgeV1::decode(decoder))
            })?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreMirBridgeBranchV1 {
    NotCore,
    Core(CoreMirBridgeV1),
}

impl WireEncode for CoreMirBridgeBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NotCore => encode_empty_sum(encoder, 1),
            Self::Core(bridge) => encode_value_sum(encoder, 2, bridge),
        }
    }
}

#[derive(Debug)]
enum DecodedCoreMirBridgeBranchV1 {
    NotCore,
    Core(DecodedCoreMirBridgeV1),
}

impl WireEncode for DecodedCoreMirBridgeBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NotCore => encode_empty_sum(encoder, 1),
            Self::Core(bridge) => encode_value_sum(encoder, 2, bridge),
        }
    }
}

impl WireDecode for DecodedCoreMirBridgeBranchV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decode_sum(decoder, |tag, decoder| match tag {
            1 => Ok(Self::NotCore),
            2 => DecodedCoreMirBridgeV1::decode(decoder).map(Self::Core),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        })
    }
}

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
    core_bridge: CoreMirBridgeBranchV1,
    entry_bridge: EntryMirBridgeBranchV1,
    strong_callable_bridges: StrongCallableBridgeSurfaceV1,
}

impl CoreBootstrapBridgeSectionV1 {
    pub fn try_new(
        artifact: scoop_identity::ConeIdentity,
        core_bridge: CoreMirBridgeBranchV1,
        entry_bridge: EntryMirBridgeBranchV1,
        strong_callable_bridges: StrongCallableBridgeSurfaceV1,
    ) -> Result<Self, MirProductionBuildError> {
        validate_section_relations(
            artifact,
            &core_bridge,
            &entry_bridge,
            &strong_callable_bridges,
        )?;
        Ok(Self {
            core_bridge,
            entry_bridge,
            strong_callable_bridges,
        })
    }

    pub const fn core_bridge(&self) -> &CoreMirBridgeBranchV1 {
        &self.core_bridge
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
        encoder.map(3)?;
        encoder.field(1)?;
        self.core_bridge.encode(encoder)?;
        encoder.field(2)?;
        self.entry_bridge.encode(encoder)?;
        encoder.field(3)?;
        self.strong_callable_bridges.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedCoreBootstrapBridgeSectionV1 {
    core_bridge: DecodedCoreMirBridgeBranchV1,
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
        let strong_callable_bridges = self
            .strong_callable_bridges
            .validate(identities, foundation.canonical())?;
        let core_bridge = match self.core_bridge {
            DecodedCoreMirBridgeBranchV1::NotCore => CoreMirBridgeBranchV1::NotCore,
            DecodedCoreMirBridgeBranchV1::Core(bridge) => {
                CoreMirBridgeBranchV1::Core(bridge.validate(identities)?)
            }
        };
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
        validate_section_relations(
            artifact,
            &core_bridge,
            &entry_bridge,
            &strong_callable_bridges,
        )
        .map_err(MirProductionValidationError::Relation)?;
        Ok(CoreBootstrapBridgeSectionV1 {
            core_bridge,
            entry_bridge,
            strong_callable_bridges,
        })
    }
}

impl WireEncode for DecodedCoreBootstrapBridgeSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.core_bridge.encode(encoder)?;
        encoder.field(2)?;
        self.entry_bridge.encode(encoder)?;
        encoder.field(3)?;
        self.strong_callable_bridges.encode(encoder)
    }
}

impl WireDecode for DecodedCoreBootstrapBridgeSectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            core_bridge: decoder.field(1, DecodedCoreMirBridgeBranchV1::decode)?,
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
    core_bridge: &CoreMirBridgeBranchV1,
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
    match (artifact == scoop_identity::ConeIdentity::CORE, core_bridge) {
        (true, CoreMirBridgeBranchV1::NotCore) => {
            return Err(MirProductionBuildError::MissingCoreBridge);
        }
        (false, CoreMirBridgeBranchV1::Core(_)) => {
            return Err(MirProductionBuildError::UnexpectedCoreBridge(artifact));
        }
        (true, CoreMirBridgeBranchV1::Core(_)) | (false, CoreMirBridgeBranchV1::NotCore) => {}
    }
    if artifact == scoop_identity::ConeIdentity::CORE
        && entry_bridge != &EntryMirBridgeBranchV1::Library
    {
        return Err(MirProductionBuildError::CoreMustBeLibrary);
    }
    if let CoreMirBridgeBranchV1::Core(core) = core_bridge {
        for bridge in &core.callable_targets {
            if strong_callable_bridges.get(bridge.implementation).is_none() {
                return Err(MirProductionBuildError::MissingStrongCoreImplementation(
                    bridge.implementation,
                ));
            }
        }
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
    DuplicateCoreBinding(PersistentExportBindingId),
    DuplicateCoreImplementation {
        index: usize,
        implementation: CallableOwner,
    },
    CoreImplementationMismatch {
        definition: PersistentFunctionId,
        implementation: CallableOwner,
    },
    EntryImplementationMismatch {
        source_entry: PersistentFunctionId,
        implementation: CallableOwner,
    },
    ForeignEntrySource {
        artifact: scoop_identity::ConeIdentity,
        entry: scoop_identity::ConeIdentity,
    },
    MissingCoreBridge,
    UnexpectedCoreBridge(scoop_identity::ConeIdentity),
    CoreMustBeLibrary,
    MissingStrongCoreImplementation(CallableOwner),
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
    DuplicateCoreBinding(PersistentExportBindingId),
    NonCanonicalCoreBindingOrder {
        index: usize,
    },
    DuplicateCoreImplementation {
        index: usize,
        implementation: CallableOwner,
    },
    CoreImplementationMismatch {
        definition: PersistentFunctionId,
        implementation: CallableOwner,
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
