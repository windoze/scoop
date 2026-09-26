//! Complete strong callable records and their shared semantic roles.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongCallableBridgeV1 {
    pub(super) implementation: CallableOwner,
    pub(super) signature: ExactCallableSignature,
    pub(super) role: crate::CallableRole,
}

impl StrongCallableBridgeV1 {
    pub const fn new(implementation: CallableOwner, signature: ExactCallableSignature) -> Self {
        Self {
            implementation,
            signature,
            role: crate::CallableRole::Ordinary,
        }
    }

    pub const fn role(&self) -> crate::CallableRole {
        self.role
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
        encoder.map(3)?;
        encoder.field(1)?;
        self.implementation.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)?;
        encoder.field(3)?;
        encode_callable_role(self.role, encoder)
    }
}

#[derive(Debug)]
pub(super) struct DecodedStrongCallableBridgeV1 {
    implementation: DecodedCallableOwner,
    signature: DecodedExactCallableSignature,
    role: crate::CallableRole,
}

impl WireEncode for DecodedStrongCallableBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.implementation.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)?;
        encoder.field(3)?;
        encode_callable_role(self.role, encoder)
    }
}

impl WireDecode for DecodedStrongCallableBridgeV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            implementation: decoder.field(1, DecodedCallableOwner::decode)?,
            signature: decoder.field(2, DecodedExactCallableSignature::decode)?,
            role: decoder.field(3, decode_callable_role)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongCallableBridgeSurfaceV1 {
    pub(super) bridges: Vec<StrongCallableBridgeV1>,
}

impl StrongCallableBridgeSurfaceV1 {
    pub fn from_odr_free_foundation(foundation: &OdrFreeMirFoundation) -> Self {
        let mut bridges = foundation
            .as_canonical()
            .callable_signatures()
            .iter()
            .map(|record| {
                let CallableSignatureSubject::Strong(implementation) = record.subject() else {
                    unreachable!("OdrFreeMirFoundation excludes ODR signature subjects")
                };
                StrongCallableBridgeV1::new(implementation, record.signature().clone())
            })
            .collect::<Vec<_>>();
        bridges.sort_unstable_by_key(StrongCallableBridgeV1::implementation);
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
        validate_callable_roles(&bridges)?;
        Ok(Self { bridges })
    }

    pub fn with_initialization_cycle(
        mut self,
        definition: PersistentFunctionId,
    ) -> Result<Self, MirProductionBuildError> {
        if self.initialization_cycle().is_some() {
            return Err(MirProductionBuildError::DuplicateInitializationCycle);
        }
        let implementation = CallableOwner::Function(definition);
        let index = self
            .bridges
            .binary_search_by_key(&implementation, StrongCallableBridgeV1::implementation)
            .map_err(|_| {
                MirProductionBuildError::MissingStrongInitializationCycle(implementation)
            })?;
        self.bridges[index].role = crate::CallableRole::InitializationCycle;
        Ok(self)
    }

    pub fn initialization_cycle(&self) -> Option<&StrongCallableBridgeV1> {
        self.bridges
            .iter()
            .find(|bridge| bridge.role == crate::CallableRole::InitializationCycle)
    }

    pub fn matches_foundation(&self, foundation: &OdrFreeMirFoundation) -> bool {
        let signatures = foundation.as_canonical().callable_signatures();
        self.bridges.len() == signatures.len()
            && self.bridges.iter().all(|actual| {
                foundation
                    .as_canonical()
                    .callable_signature(actual.subject())
                    .is_some_and(|expected| actual.signature() == expected.signature())
            })
    }

    pub fn bridges(&self) -> &[StrongCallableBridgeV1] {
        &self.bridges
    }

    pub fn get(&self, implementation: CallableOwner) -> Option<&StrongCallableBridgeV1> {
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
pub(super) struct DecodedStrongCallableBridgeSurfaceV1 {
    pub(super) bridges: Vec<DecodedStrongCallableBridgeV1>,
}

impl DecodedStrongCallableBridgeSurfaceV1 {
    pub(super) fn validate(
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
        if foundation
            .callable_signatures()
            .iter()
            .any(|record| matches!(record.subject(), CallableSignatureSubject::Odr(_)))
        {
            return Err(MirProductionValidationError::FoundationOdrSubject);
        }
        for (index, bridge) in bridges.iter().enumerate() {
            let subject = bridge.subject();
            let Ok(expected_index) = foundation
                .callable_signatures()
                .binary_search_by(|record| record.subject().compare_sort_key(subject))
            else {
                return Err(MirProductionValidationError::StrongCallableMismatch { index });
            };
            let expected = &foundation.callable_signatures()[expected_index];
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
        validate_callable_roles(&bridges).map_err(MirProductionValidationError::Relation)?;
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
        role: decoded.role,
    })
}

fn encode_callable_role(
    role: crate::CallableRole,
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.unsigned(match role {
        crate::CallableRole::Ordinary => 1,
        crate::CallableRole::InitializationCycle => 2,
    })
}
fn decode_callable_role(decoder: &mut Decoder<'_>) -> Result<crate::CallableRole, WireError> {
    match decoder.unsigned()? {
        1 => Ok(crate::CallableRole::Ordinary),
        2 => Ok(crate::CallableRole::InitializationCycle),
        tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}
fn validate_callable_roles(
    bridges: &[StrongCallableBridgeV1],
) -> Result<(), MirProductionBuildError> {
    let mut has_initialization_cycle = false;
    for bridge in bridges {
        if bridge.role != crate::CallableRole::InitializationCycle {
            continue;
        }
        if !matches!(bridge.implementation, CallableOwner::Function(_)) {
            return Err(MirProductionBuildError::InvalidInitializationCycleOwner(
                bridge.implementation,
            ));
        }
        if has_initialization_cycle {
            return Err(MirProductionBuildError::DuplicateInitializationCycle);
        }
        has_initialization_cycle = true;
    }
    Ok(())
}
