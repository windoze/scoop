use std::fmt;

use scoop_identity::{
    ArtifactCapabilityProfileId, ConeCoordinate, RequestedConeKind, SourceContentDigest,
    SourceIdentity,
};
use scoop_lir::{BackendProfileFingerprint, CBridgeToolchainFingerprint, TargetProfileFingerprint};
use scoop_protocol::ScoopcProtocolCapabilityV1;
use scoop_slib::{
    ArtifactCapabilityProfile, CodeFingerprint, ConeRecord, HirFingerprint, IdentityAbiDescriptor,
    LirFingerprint, MirFingerprint,
};
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use crate::PairedCompilerFingerprintV1;

const CACHE_KEY_DOMAIN: &str = "scoop-cone-compile-cache-v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConeCompileCacheKeyV1([u8; 32]);

impl ConeCompileCacheKeyV1 {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }

    pub(crate) const fn from_digest(digest: scoop_wire::Digest256) -> Self {
        Self(*digest.as_array())
    }
}

impl WireEncode for ConeCompileCacheKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for ConeCompileCacheKeyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCacheInputV1 {
    source: SourceIdentity,
    content: SourceContentDigest,
}

impl SourceCacheInputV1 {
    pub(crate) const fn new(source: SourceIdentity, content: SourceContentDigest) -> Self {
        Self { source, content }
    }

    pub const fn source(&self) -> &SourceIdentity {
        &self.source
    }

    pub const fn content(&self) -> SourceContentDigest {
        self.content
    }
}

impl WireEncode for SourceCacheInputV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.source.encode(encoder)?;
        encoder.field(2)?;
        self.content.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionalCoreCodeFingerprintV1 {
    None,
    Some(CodeFingerprint),
}

impl WireEncode for OptionalCoreCodeFingerprintV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Some(fingerprint) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                fingerprint.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileDependencyInputV1 {
    cone: ConeRecord,
    hir: HirFingerprint,
    mir: MirFingerprint,
    lir: LirFingerprint,
    single_file_core_code: OptionalCoreCodeFingerprintV1,
}

impl CompileDependencyInputV1 {
    pub(crate) const fn new(
        cone: ConeRecord,
        hir: HirFingerprint,
        mir: MirFingerprint,
        lir: LirFingerprint,
        single_file_core_code: OptionalCoreCodeFingerprintV1,
    ) -> Self {
        Self {
            cone,
            hir,
            mir,
            lir,
            single_file_core_code,
        }
    }

    pub const fn cone(&self) -> &ConeRecord {
        &self.cone
    }

    pub const fn hir(&self) -> HirFingerprint {
        self.hir
    }

    pub const fn mir(&self) -> MirFingerprint {
        self.mir
    }

    pub const fn lir(&self) -> LirFingerprint {
        self.lir
    }

    pub const fn single_file_core_code(&self) -> OptionalCoreCodeFingerprintV1 {
        self.single_file_core_code
    }
}

impl WireEncode for CompileDependencyInputV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.cone.encode(encoder)?;
        encoder.field(2)?;
        self.hir.encode(encoder)?;
        encoder.field(3)?;
        self.mir.encode(encoder)?;
        encoder.field(4)?;
        self.lir.encode(encoder)?;
        encoder.field(5)?;
        self.single_file_core_code.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CurrentConeSemanticProjectionV1 {
    Manifest {
        coordinate: ConeCoordinate,
        requested_kind: RequestedConeKind,
        dependencies: Vec<ConeCoordinate>,
    },
    SingleFile,
}

impl WireEncode for CurrentConeSemanticProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Manifest {
                coordinate,
                requested_kind,
                dependencies,
            } => {
                encoder.map(4)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                coordinate.encode(encoder)?;
                encoder.field(2)?;
                encode_requested_kind(*requested_kind, encoder)?;
                encoder.field(3)?;
                encode_array(dependencies, encoder)
            }
            Self::SingleFile => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                ConeCoordinate::reserved_single_file().encode(encoder)?;
                encoder.field(2)?;
                scoop_identity::NormalizedSourcePath::single_file().encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeCompileCacheInputV1 {
    cone: ConeRecord,
    current_semantic: CurrentConeSemanticProjectionV1,
    sources: Vec<SourceCacheInputV1>,
    dependencies: Vec<CompileDependencyInputV1>,
    compiler: PairedCompilerFingerprintV1,
    compatibility: IdentityAbiDescriptor,
    artifact_profile: ArtifactCapabilityProfileId,
    protocol: ScoopcProtocolCapabilityV1,
    lir_target: TargetProfileFingerprint,
    backend: BackendProfileFingerprint,
    c_bridge_toolchain: CBridgeToolchainFingerprint,
}

impl ConeCompileCacheInputV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        cone: ConeRecord,
        current_semantic: CurrentConeSemanticProjectionV1,
        sources: Vec<SourceCacheInputV1>,
        dependencies: Vec<CompileDependencyInputV1>,
        compiler: PairedCompilerFingerprintV1,
        compatibility: IdentityAbiDescriptor,
        artifact_profile: ArtifactCapabilityProfileId,
        protocol: ScoopcProtocolCapabilityV1,
        lir_target: TargetProfileFingerprint,
        backend: BackendProfileFingerprint,
        c_bridge_toolchain: CBridgeToolchainFingerprint,
    ) -> Self {
        Self {
            cone,
            current_semantic,
            sources,
            dependencies,
            compiler,
            compatibility,
            artifact_profile,
            protocol,
            lir_target,
            backend,
            c_bridge_toolchain,
        }
    }

    pub fn key(&self) -> Result<ConeCompileCacheKeyV1, HashError> {
        domain_separated_cbor_hash(CACHE_KEY_DOMAIN, self)
            .map(|digest| ConeCompileCacheKeyV1(*digest.as_array()))
    }

    pub const fn cone(&self) -> &ConeRecord {
        &self.cone
    }

    pub const fn current_semantic(&self) -> &CurrentConeSemanticProjectionV1 {
        &self.current_semantic
    }

    pub fn sources(&self) -> &[SourceCacheInputV1] {
        &self.sources
    }

    pub fn dependencies(&self) -> &[CompileDependencyInputV1] {
        &self.dependencies
    }

    pub const fn compiler(&self) -> PairedCompilerFingerprintV1 {
        self.compiler
    }

    pub const fn protocol(&self) -> ScoopcProtocolCapabilityV1 {
        self.protocol
    }
}

impl WireEncode for ConeCompileCacheInputV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encoder.field(1)?;
        encoder.unsigned(1)?;
        encoder.field(2)?;
        self.cone.encode(encoder)?;
        encoder.field(3)?;
        self.current_semantic.encode(encoder)?;
        encoder.field(4)?;
        encode_array(&self.sources, encoder)?;
        encoder.field(5)?;
        encode_array(&self.dependencies, encoder)?;
        encoder.field(6)?;
        self.compiler.encode(encoder)?;
        encoder.field(7)?;
        self.compatibility.encode(encoder)?;
        encoder.field(8)?;
        self.artifact_profile.encode(encoder)?;
        encoder.field(9)?;
        self.protocol.encode(encoder)?;
        encoder.field(10)?;
        self.lir_target.encode(encoder)?;
        encoder.field(11)?;
        self.backend.encode(encoder)?;
        encoder.field(12)?;
        self.c_bridge_toolchain.encode(encoder)
    }
}

pub(crate) fn strong_profile_id() -> ArtifactCapabilityProfileId {
    ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG.id()
}

fn encode_requested_kind(
    kind: RequestedConeKind,
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.unsigned(match kind {
        RequestedConeKind::Library => 1,
        RequestedConeKind::Executable => 2,
    })
}

fn encode_array<T: WireEncode>(
    values: &[T],
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
