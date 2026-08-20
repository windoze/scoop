use super::*;

pub(super) struct CanonicalSemanticContribution<'section> {
    pub(super) location: MetadataLocation,
    pub(super) capability: &'section CapabilityId,
    pub(super) sink: FingerprintSink,
    pub(super) projection: &'section [u8],
}

impl WireEncode for CanonicalSemanticContribution<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        encoder.unsigned(match self.location {
            MetadataLocation::Hir => 2,
            MetadataLocation::Mir => 3,
            MetadataLocation::Lir => 4,
        })?;
        encoder.field(2)?;
        self.capability.encode(encoder)?;
        encoder.field(3)?;
        self.sink.encode(encoder)?;
        encoder.field(4)?;
        encoder.bytes(self.projection)
    }
}

pub(super) struct LayerFingerprintContext<'compatibility> {
    pub(super) layer: FoundationLayer,
    pub(super) compatibility: &'compatibility CompatibilityRecord,
}

impl WireEncode for LayerFingerprintContext<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.layer {
            FoundationLayer::Hir => {
                encoder.map(2)?;
                encode_layer_tag(encoder, 1)?;
                encoder.field(1)?;
                self.compatibility.composite_identity_abi().encode(encoder)
            }
            FoundationLayer::Mir => {
                encoder.map(3)?;
                encode_layer_tag(encoder, 2)?;
                encoder.field(1)?;
                self.compatibility
                    .composite_identity_abi()
                    .encode(encoder)?;
                encoder.field(2)?;
                self.compatibility.mangling_schema().encode(encoder)
            }
            FoundationLayer::Lir => {
                encoder.map(4)?;
                encode_layer_tag(encoder, 3)?;
                encoder.field(1)?;
                self.compatibility
                    .composite_identity_abi()
                    .encode(encoder)?;
                encoder.field(2)?;
                self.compatibility.target_profile().encode(encoder)?;
                encoder.field(3)?;
                self.compatibility.target_fingerprint().encode(encoder)
            }
        }
    }
}

fn encode_layer_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

pub(super) struct SupportEdges<'dependency> {
    pub(super) layer: FoundationLayer,
    pub(super) dependencies: &'dependency OrderedDependencies<'dependency>,
}

impl WireEncode for SupportEdges<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.dependencies.len() as u64)?;
        let role = SupportEdgeRoleId::all_direct();
        match &self.dependencies.entries {
            DependencyEntries::Canonical(entries) => {
                for dependency in *entries {
                    encode_support_edge(encoder, self.layer, &role, dependency)?;
                }
            }
            DependencyEntries::Sorted(entries) => {
                for dependency in entries {
                    encode_support_edge(encoder, self.layer, &role, dependency)?;
                }
            }
        }
        Ok(())
    }
}

fn encode_support_edge(
    encoder: &mut Encoder,
    layer: FoundationLayer,
    role: &SupportEdgeRoleId,
    dependency: &DependencyRecord,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(1)?;
    dependency.identity().encode(encoder)?;
    encoder.field(2)?;
    role.encode(encoder)?;
    encoder.field(3)?;
    match layer {
        FoundationLayer::Hir => dependency.hir_fingerprint().encode(encoder),
        FoundationLayer::Mir => dependency.mir_fingerprint().encode(encoder),
        FoundationLayer::Lir => dependency.lir_fingerprint().encode(encoder),
    }
}

pub(super) struct LayerFingerprintInput<'input> {
    pub(super) context: LayerFingerprintContext<'input>,
    pub(super) contributions: Vec<CanonicalSemanticContribution<'input>>,
    pub(super) support_edges: SupportEdges<'input>,
}

impl WireEncode for LayerFingerprintInput<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.context.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.contributions.len() as u64)?;
        for contribution in &self.contributions {
            contribution.encode(encoder)?;
        }
        encoder.field(3)?;
        self.support_edges.encode(encoder)
    }
}
