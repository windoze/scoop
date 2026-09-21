//! Initialization protocol and nominal support in core MIR metadata.

use super::*;

/// Required strong implementation of the core-internal initialization cycle
/// service. It has no public export binding and therefore cannot enter source
/// prelude lookup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreMirInitializationCycleThrowerV1 {
    definition: PersistentFunctionId,
    implementation: CallableOwner,
}

impl CoreMirInitializationCycleThrowerV1 {
    pub fn new(
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
            definition,
            implementation,
        })
    }

    pub const fn definition(self) -> PersistentFunctionId {
        self.definition
    }

    pub const fn implementation(self) -> CallableOwner {
        self.implementation
    }
}

impl WireEncode for CoreMirInitializationCycleThrowerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.definition.encode(encoder)?;
        encoder.field(2)?;
        self.implementation.encode(encoder)
    }
}

#[derive(Debug)]
struct DecodedCoreMirInitializationCycleThrowerV1 {
    definition: DecodedPersistentId<PersistentFunctionId>,
    implementation: DecodedCallableOwner,
}

impl WireEncode for DecodedCoreMirInitializationCycleThrowerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.definition.encode(encoder)?;
        encoder.field(2)?;
        self.implementation.encode(encoder)
    }
}

impl WireDecode for DecodedCoreMirInitializationCycleThrowerV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            definition: decoder.field(1, DecodedPersistentId::decode)?,
            implementation: decoder.field(2, DecodedCallableOwner::decode)?,
        })
    }
}

/// One source-nominal root whose finite strong shape-support closure must be
/// materialized by the trusted core producer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreMirShapeSupportRootV1 {
    source: PersistentTypeId,
    exact: PersistentExactTypeId,
}

impl CoreMirShapeSupportRootV1 {
    pub fn new(
        source: PersistentTypeId,
        exact: PersistentExactTypeId,
    ) -> Result<Self, MirProductionBuildError> {
        validate_core_shape_exact(source, exact).map_err(|error| match error {
            CoreShapeExactError::Identity(source) => {
                MirProductionBuildError::CoreShapeExactIdentity(source)
            }
            CoreShapeExactError::Mismatch {
                source,
                expected,
                actual,
            } => MirProductionBuildError::CoreShapeExactMismatch {
                source,
                expected,
                actual,
            },
        })?;
        Ok(Self { source, exact })
    }

    pub const fn source(self) -> PersistentTypeId {
        self.source
    }

    pub const fn exact(self) -> PersistentExactTypeId {
        self.exact
    }
}

enum CoreShapeExactError {
    Identity(scoop_wire::HashError),
    Mismatch {
        source: PersistentTypeId,
        expected: PersistentExactTypeId,
        actual: PersistentExactTypeId,
    },
}

fn validate_core_shape_exact(
    source: PersistentTypeId,
    exact: PersistentExactTypeId,
) -> Result<(), CoreShapeExactError> {
    let expected = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source))
        .map_err(CoreShapeExactError::Identity)?;
    if exact != expected {
        return Err(CoreShapeExactError::Mismatch {
            source,
            expected,
            actual: exact,
        });
    }
    Ok(())
}

impl WireEncode for CoreMirShapeSupportRootV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.source.encode(encoder)?;
        encoder.field(2)?;
        self.exact.encode(encoder)
    }
}

#[derive(Debug)]
struct DecodedCoreMirShapeSupportRootV1 {
    source: DecodedPersistentId<PersistentTypeId>,
    exact: DecodedPersistentId<PersistentExactTypeId>,
}

impl WireEncode for DecodedCoreMirShapeSupportRootV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.source.encode(encoder)?;
        encoder.field(2)?;
        self.exact.encode(encoder)
    }
}

impl WireDecode for DecodedCoreMirShapeSupportRootV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            source: decoder.field(1, DecodedPersistentId::decode)?,
            exact: decoder.field(2, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreMirBridgeV1 {
    shape_support_roots: Vec<CoreMirShapeSupportRootV1>,
    initialization_cycle_thrower: CoreMirInitializationCycleThrowerV1,
}

impl CoreMirBridgeV1 {
    pub fn try_new(
        mut shape_support_roots: Vec<CoreMirShapeSupportRootV1>,
        initialization_cycle_thrower: CoreMirInitializationCycleThrowerV1,
    ) -> Result<Self, MirProductionBuildError> {
        shape_support_roots.sort_unstable_by_key(|root| root.source);
        if let Some(pair) = shape_support_roots
            .windows(2)
            .find(|pair| pair[0].source == pair[1].source)
        {
            return Err(MirProductionBuildError::DuplicateCoreShapeSource(
                pair[0].source,
            ));
        }
        Ok(Self {
            shape_support_roots,
            initialization_cycle_thrower,
        })
    }

    pub fn shape_support_roots(&self) -> &[CoreMirShapeSupportRootV1] {
        &self.shape_support_roots
    }

    pub const fn initialization_cycle_thrower(&self) -> CoreMirInitializationCycleThrowerV1 {
        self.initialization_cycle_thrower
    }
}

impl WireEncode for CoreMirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(2)?;
        encoder.array(self.shape_support_roots.len() as u64)?;
        for root in &self.shape_support_roots {
            root.encode(encoder)?;
        }
        encoder.field(3)?;
        self.initialization_cycle_thrower.encode(encoder)
    }
}

#[derive(Debug)]
pub(super) struct DecodedCoreMirBridgeV1 {
    shape_support_roots: Vec<DecodedCoreMirShapeSupportRootV1>,
    initialization_cycle_thrower: DecodedCoreMirInitializationCycleThrowerV1,
}

impl DecodedCoreMirBridgeV1 {
    pub(super) fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CoreMirBridgeV1, MirProductionValidationError> {
        let mut shape_support_roots: Vec<CoreMirShapeSupportRootV1> =
            Vec::with_capacity(self.shape_support_roots.len());
        for (index, decoded) in self.shape_support_roots.into_iter().enumerate() {
            let source = identities
                .resolve(decoded.source)
                .map_err(MirProductionValidationError::Identity)?;
            let exact = identities
                .resolve(decoded.exact)
                .map_err(MirProductionValidationError::Identity)?;
            validate_core_shape_exact(source, exact).map_err(|error| match error {
                CoreShapeExactError::Identity(source) => {
                    MirProductionValidationError::CoreShapeExactIdentity(source)
                }
                CoreShapeExactError::Mismatch {
                    source,
                    expected,
                    actual,
                } => MirProductionValidationError::CoreShapeExactMismatch {
                    source,
                    expected,
                    actual,
                },
            })?;
            let root = CoreMirShapeSupportRootV1 { source, exact };
            if index > 0 && shape_support_roots[index - 1].source >= root.source {
                return Err(if shape_support_roots[index - 1].source == root.source {
                    MirProductionValidationError::DuplicateCoreShapeSource(root.source)
                } else {
                    MirProductionValidationError::NonCanonicalCoreShapeSourceOrder { index }
                });
            }
            shape_support_roots.push(root);
        }
        let definition = identities
            .resolve(self.initialization_cycle_thrower.definition)
            .map_err(MirProductionValidationError::Identity)?;
        let implementation = self
            .initialization_cycle_thrower
            .implementation
            .resolve(identities)
            .map_err(MirProductionValidationError::Identity)?;
        let initialization_cycle_thrower =
            CoreMirInitializationCycleThrowerV1::new(definition, implementation)
                .map_err(MirProductionValidationError::Relation)?;
        Ok(CoreMirBridgeV1 {
            shape_support_roots,
            initialization_cycle_thrower,
        })
    }
}

impl WireEncode for DecodedCoreMirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(2)?;
        encoder.array(self.shape_support_roots.len() as u64)?;
        for root in &self.shape_support_roots {
            root.encode(encoder)?;
        }
        encoder.field(3)?;
        self.initialization_cycle_thrower.encode(encoder)
    }
}

impl WireDecode for DecodedCoreMirBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            shape_support_roots: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedCoreMirShapeSupportRootV1::decode(decoder))
            })?,
            initialization_cycle_thrower: decoder
                .field(3, DecodedCoreMirInitializationCycleThrowerV1::decode)?,
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
pub(super) enum DecodedCoreMirBridgeBranchV1 {
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
