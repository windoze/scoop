//! Persistent identity records introduced by the LIR foundation.

use std::fmt;
use std::num::NonZeroU64;

use scoop_identity::{
    CanonicalCAbiSignatureFingerprint, DecodedPersistentId, DerivedIdError, GeneratedBridgeUnitId,
    PersistentCallbackApplicationId, PersistentExactTypeId, PersistentIdResolver,
    PersistentSafepointSiteId, RuntimeTypeId, SafepointId as PersistentSafepointId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// Canonical target-specific C storage signatures and layouts required by
/// this LIR module. Repeated boundary uses share one record; a digest collision
/// with a different preimage is rejected before the metadata becomes visible.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalCAbiMetadata {
    signatures: Vec<scoop_identity::CanonicalCAbiSignatureFingerprintRecord>,
    layouts: Vec<scoop_identity::CanonicalCAbiLayoutFingerprintRecord>,
}

impl CanonicalCAbiMetadata {
    pub fn checked(
        mut signatures: Vec<scoop_identity::CanonicalCAbiSignatureFingerprintRecord>,
        mut layouts: Vec<scoop_identity::CanonicalCAbiLayoutFingerprintRecord>,
    ) -> Result<Self, CanonicalCAbiMetadataError> {
        signatures.sort_by_key(|record| record.fingerprint());
        signatures.dedup();
        if signatures
            .windows(2)
            .any(|pair| pair[0].fingerprint() == pair[1].fingerprint())
        {
            return Err(CanonicalCAbiMetadataError::SignatureCollision);
        }

        layouts.sort_by_key(|record| record.fingerprint());
        layouts.dedup();
        if layouts
            .windows(2)
            .any(|pair| pair[0].fingerprint() == pair[1].fingerprint())
        {
            return Err(CanonicalCAbiMetadataError::LayoutCollision);
        }

        Ok(Self {
            signatures,
            layouts,
        })
    }

    pub fn signatures(&self) -> &[scoop_identity::CanonicalCAbiSignatureFingerprintRecord] {
        &self.signatures
    }

    pub fn layouts(&self) -> &[scoop_identity::CanonicalCAbiLayoutFingerprintRecord] {
        &self.layouts
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalCAbiMetadataError {
    SignatureCollision,
    LayoutCollision,
}

impl fmt::Display for CanonicalCAbiMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SignatureCollision => {
                "different canonical C ABI signatures have the same fingerprint"
            }
            Self::LayoutCollision => "different canonical C ABI layouts have the same fingerprint",
        })
    }
}

impl std::error::Error for CanonicalCAbiMetadataError {}

/// The deterministic runtime type id derived from one persistent exact type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeTypeMappingRecord {
    exact_type: PersistentExactTypeId,
    runtime_type: RuntimeTypeId,
}

impl RuntimeTypeMappingRecord {
    pub fn new(exact_type: PersistentExactTypeId) -> Result<Self, DerivedIdError> {
        let runtime_type = RuntimeTypeId::derive(exact_type)?;
        Ok(Self {
            exact_type,
            runtime_type,
        })
    }

    pub const fn exact_type(self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn runtime_type(self) -> RuntimeTypeId {
        self.runtime_type
    }
}

impl WireEncode for RuntimeTypeMappingRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.exact_type.encode(encoder)?;
        encoder.field(2)?;
        self.runtime_type.encode(encoder)
    }
}

/// The deterministic runtime safepoint id derived from one persistent site.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SafepointMappingRecord {
    site: PersistentSafepointSiteId,
    safepoint: PersistentSafepointId,
}

impl SafepointMappingRecord {
    pub fn new(site: PersistentSafepointSiteId) -> Result<Self, DerivedIdError> {
        let safepoint = PersistentSafepointId::derive(site)?;
        Ok(Self { site, safepoint })
    }

    /// Preserve the already-validated full-to-runtime relation carried by a
    /// final LIR safepoint identity.
    pub fn from_identity(identity: &crate::SafepointIdentity) -> Self {
        Self {
            site: identity.site_id(),
            safepoint: identity.runtime_id(),
        }
    }

    pub const fn site(self) -> PersistentSafepointSiteId {
        self.site
    }

    pub const fn safepoint(self) -> PersistentSafepointId {
        self.safepoint
    }
}

impl WireEncode for SafepointMappingRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.site.encode(encoder)?;
        encoder.field(2)?;
        self.safepoint.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallbackBridgeRecord {
    application: PersistentCallbackApplicationId,
    signature: CanonicalCAbiSignatureFingerprint,
    unit: GeneratedBridgeUnitId,
}

impl CallbackBridgeRecord {
    pub const fn new(
        application: PersistentCallbackApplicationId,
        signature: CanonicalCAbiSignatureFingerprint,
        unit: GeneratedBridgeUnitId,
    ) -> Self {
        Self {
            application,
            signature,
            unit,
        }
    }

    pub const fn application(self) -> PersistentCallbackApplicationId {
        self.application
    }

    pub const fn signature(self) -> CanonicalCAbiSignatureFingerprint {
        self.signature
    }

    pub const fn unit(self) -> GeneratedBridgeUnitId {
        self.unit
    }
}

impl WireEncode for CallbackBridgeRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.application.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)?;
        encoder.field(3)?;
        self.unit.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedRuntimeTypeMappingRecord {
    exact_type: DecodedPersistentId<PersistentExactTypeId>,
    runtime_type: NonZeroU64,
}

impl DecodedRuntimeTypeMappingRecord {
    pub const fn decoded_exact_type(&self) -> DecodedPersistentId<PersistentExactTypeId> {
        self.exact_type
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<RuntimeTypeMappingRecord, RuntimeTypeMappingResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        let exact_type = resolver
            .resolve(self.exact_type)
            .map_err(RuntimeTypeMappingResolutionError::Reference)?;
        let record = RuntimeTypeMappingRecord::new(exact_type)
            .map_err(RuntimeTypeMappingResolutionError::Derivation)?;
        if record.runtime_type().get() != self.runtime_type.get() {
            return Err(RuntimeTypeMappingResolutionError::Mismatch {
                expected: record.runtime_type().get(),
                actual: self.runtime_type.get(),
            });
        }
        Ok(record)
    }
}

impl WireEncode for DecodedRuntimeTypeMappingRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.exact_type.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(self.runtime_type.get())
    }
}

impl WireDecode for DecodedRuntimeTypeMappingRecord {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            exact_type: decoder.field(1, DecodedPersistentId::decode)?,
            runtime_type: decoder.field(2, decode_non_zero_u64)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedSafepointMappingRecord {
    site: DecodedPersistentId<PersistentSafepointSiteId>,
    safepoint: NonZeroU64,
}

impl DecodedSafepointMappingRecord {
    pub const fn decoded_site(&self) -> DecodedPersistentId<PersistentSafepointSiteId> {
        self.site
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<SafepointMappingRecord, SafepointMappingResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentSafepointSiteId, Error = E>,
    {
        let site = resolver
            .resolve(self.site)
            .map_err(SafepointMappingResolutionError::Reference)?;
        let record = SafepointMappingRecord::new(site)
            .map_err(SafepointMappingResolutionError::Derivation)?;
        if record.safepoint().get() != self.safepoint.get() {
            return Err(SafepointMappingResolutionError::Mismatch {
                expected: record.safepoint().get(),
                actual: self.safepoint.get(),
            });
        }
        Ok(record)
    }
}

impl WireEncode for DecodedSafepointMappingRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.site.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(self.safepoint.get())
    }
}

impl WireDecode for DecodedSafepointMappingRecord {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            site: decoder.field(1, DecodedPersistentId::decode)?,
            safepoint: decoder.field(2, decode_non_zero_u64)?,
        })
    }
}

pub trait CallbackBridgeResolver<E>:
    PersistentIdResolver<PersistentCallbackApplicationId, Error = E>
    + PersistentIdResolver<CanonicalCAbiSignatureFingerprint, Error = E>
    + PersistentIdResolver<GeneratedBridgeUnitId, Error = E>
{
}

impl<T, E> CallbackBridgeResolver<E> for T where
    T: PersistentIdResolver<PersistentCallbackApplicationId, Error = E>
        + PersistentIdResolver<CanonicalCAbiSignatureFingerprint, Error = E>
        + PersistentIdResolver<GeneratedBridgeUnitId, Error = E>
{
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedCallbackBridgeRecord {
    application: DecodedPersistentId<PersistentCallbackApplicationId>,
    signature: DecodedPersistentId<CanonicalCAbiSignatureFingerprint>,
    unit: DecodedPersistentId<GeneratedBridgeUnitId>,
}

impl DecodedCallbackBridgeRecord {
    pub const fn decoded_application(
        &self,
    ) -> DecodedPersistentId<PersistentCallbackApplicationId> {
        self.application
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CallbackBridgeRecord, CallbackBridgeResolutionError<E>>
    where
        R: CallbackBridgeResolver<E>,
    {
        let application = resolver
            .resolve(self.application)
            .map_err(CallbackBridgeResolutionError::Application)?;
        let signature = resolver
            .resolve(self.signature)
            .map_err(CallbackBridgeResolutionError::Signature)?;
        let unit = resolver
            .resolve(self.unit)
            .map_err(CallbackBridgeResolutionError::Unit)?;
        Ok(CallbackBridgeRecord::new(application, signature, unit))
    }
}

impl WireEncode for DecodedCallbackBridgeRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.application.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)?;
        encoder.field(3)?;
        self.unit.encode(encoder)
    }
}

impl WireDecode for DecodedCallbackBridgeRecord {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            application: decoder.field(1, DecodedPersistentId::decode)?,
            signature: decoder.field(2, DecodedPersistentId::decode)?,
            unit: decoder.field(3, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeTypeMappingResolutionError<E> {
    Reference(E),
    Derivation(DerivedIdError),
    Mismatch { expected: u64, actual: u64 },
}

impl<E: fmt::Display> fmt::Display for RuntimeTypeMappingResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Derivation(error) => error.fmt(formatter),
            Self::Mismatch { expected, actual } => write!(
                formatter,
                "runtime type id mismatch: expected {expected}, found {actual}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for RuntimeTypeMappingResolutionError<E> {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SafepointMappingResolutionError<E> {
    Reference(E),
    Derivation(DerivedIdError),
    Mismatch { expected: u64, actual: u64 },
}

impl<E: fmt::Display> fmt::Display for SafepointMappingResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Derivation(error) => error.fmt(formatter),
            Self::Mismatch { expected, actual } => write!(
                formatter,
                "safepoint id mismatch: expected {expected}, found {actual}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SafepointMappingResolutionError<E> {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallbackBridgeResolutionError<E> {
    Application(E),
    Signature(E),
    Unit(E),
}

impl<E: fmt::Display> fmt::Display for CallbackBridgeResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application(error) | Self::Signature(error) | Self::Unit(error) => {
                error.fmt(formatter)
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CallbackBridgeResolutionError<E> {}

fn decode_non_zero_u64(decoder: &mut Decoder<'_, '_>) -> Result<NonZeroU64, WireError> {
    NonZeroU64::new(decoder.unsigned()?).ok_or_else(|| {
        WireError::new(
            WireErrorKind::IntegerOutOfRange,
            decoder.path().clone(),
            Some(decoder.position()),
        )
    })
}

#[cfg(test)]
mod tests;
