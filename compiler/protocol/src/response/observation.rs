use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoopcObservationV1 {
    warnings: Vec<StructuredDiagnosticV1>,
    dumps: Vec<EmittedDumpDescriptorV1>,
}

impl ScoopcObservationV1 {
    pub fn new(
        warnings: Vec<StructuredDiagnosticV1>,
        dumps: Vec<EmittedDumpDescriptorV1>,
    ) -> Result<Self, ProtocolValidationError> {
        validate_success_fields(&warnings, &dumps)?;
        if dumps.is_empty() {
            return Err(ProtocolValidationError::InvalidDumpDescriptors);
        }
        Ok(Self { warnings, dumps })
    }

    pub fn warnings(&self) -> &[StructuredDiagnosticV1] {
        &self.warnings
    }
    pub fn emitted_dump_descriptors(&self) -> &[EmittedDumpDescriptorV1] {
        &self.dumps
    }
}

pub(super) fn encode_observation(
    encoder: &mut Encoder,
    request_id: RequestCorrelationId,
    result: &ScoopcObservationV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encoder.field(0)?;
    encoder.unsigned(3)?;
    encoder.field(1)?;
    request_id.encode(encoder)?;
    encoder.field(2)?;
    encode_diagnostics(encoder, &result.warnings)?;
    encoder.field(3)?;
    encoder.array(result.dumps.len() as u64)?;
    for dump in &result.dumps {
        dump.encode(encoder)?;
    }
    Ok(())
}
