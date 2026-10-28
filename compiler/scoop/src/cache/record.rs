use scoop_protocol::StructuredDiagnosticV1;
use scoop_slib::ArtifactManifestSummaryV1;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

/// Data produced by one successful compilation and needed by later builds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileBuildRecordV1 {
    pub summary: ArtifactManifestSummaryV1,
    pub warnings: Vec<StructuredDiagnosticV1>,
    pub used_generators: Vec<scoop_protocol::GeneratorToolInputV1>,
    pub environment_inputs: Vec<scoop_protocol::EnvironmentInputV1>,
}

impl WireEncode for CompileBuildRecordV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(4)?;
        e.field(1)?;
        self.summary.encode(e)?;
        e.field(2)?;
        e.array(self.warnings.len() as u64)?;
        for warning in &self.warnings {
            warning.encode(e)?;
        }
        e.field(3)?;
        e.array(self.used_generators.len() as u64)?;
        for input in &self.used_generators {
            input.encode(e)?;
        }
        e.field(4)?;
        e.array(self.environment_inputs.len() as u64)?;
        for input in &self.environment_inputs {
            input.encode(e)?;
        }
        Ok(())
    }
}

impl WireDecode for CompileBuildRecordV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(4)?;
        Ok(Self {
            summary: d.field(1, ArtifactManifestSummaryV1::decode)?,
            warnings: d.field(2, |d| {
                d.decode_array(|d, _| StructuredDiagnosticV1::decode(d))
            })?,
            used_generators: d.field(3, |d| {
                d.decode_array(|d, _| scoop_protocol::GeneratorToolInputV1::decode(d))
            })?,
            environment_inputs: d.field(4, |d| {
                d.decode_array(|d, _| scoop_protocol::EnvironmentInputV1::decode(d))
            })?,
        })
    }
}
