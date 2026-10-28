use scoop_protocol::{GeneratorToolInputV1, StructuredDiagnosticV1};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

pub(in crate::check) struct Messages {
    pub warnings: Vec<StructuredDiagnosticV1>,
    pub used_generators: Vec<GeneratorToolInputV1>,
    pub environment_inputs: Vec<scoop_protocol::EnvironmentInputV1>,
}

impl WireEncode for Messages {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(3)?;
        encoder.array(self.warnings.len() as u64)?;
        for warning in &self.warnings {
            warning.encode(encoder)?;
        }
        encoder.array(self.used_generators.len() as u64)?;
        for input in &self.used_generators {
            input.encode(encoder)?;
        }
        encoder.array(self.environment_inputs.len() as u64)?;
        for input in &self.environment_inputs {
            input.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for Messages {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let actual = decoder.array()?;
        if actual != 3 {
            return Err(WireError::new(
                scoop_wire::WireErrorKind::InvalidLength {
                    expected: 3,
                    actual,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        Ok(Self {
            warnings: decoder.decode_array(|decoder, _| StructuredDiagnosticV1::decode(decoder))?,
            used_generators: decoder
                .decode_array(|decoder, _| GeneratorToolInputV1::decode(decoder))?,
            environment_inputs: decoder
                .decode_array(|decoder, _| scoop_protocol::EnvironmentInputV1::decode(decoder))?,
        })
    }
}
