//! Environment values observed by the compiler, retained for cache updates.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EnvironmentInputV1 {
    pub name: String,
    pub value: Option<String>,
}

impl WireEncode for EnvironmentInputV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(2)?;
        e.field(1)?;
        e.text(&self.name)?;
        e.field(2)?;
        e.array(u64::from(self.value.is_some()))?;
        if let Some(value) = &self.value {
            e.text(value)?;
        }
        Ok(())
    }
}

impl WireDecode for EnvironmentInputV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(2)?;
        let name = d.field(1, Decoder::owned_text)?;
        let value = d.field(2, |d| match d.array()? {
            0 => Ok(None),
            1 => d.owned_text().map(Some),
            actual => Err(WireError::new(
                scoop_wire::WireErrorKind::InvalidLength {
                    expected: 1,
                    actual,
                },
                d.path().clone(),
                Some(d.position()),
            )),
        })?;
        Ok(Self { name, value })
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn compile_env_response_preserves_missing_empty_and_unicode() {
        let inputs = [None, Some(""), Some("中文\nvalue")]
            .into_iter()
            .enumerate()
            .map(|(index, value)| EnvironmentInputV1 {
                name: format!("SCOOP_INPUT_{index}"),
                value: value.map(str::to_owned),
            })
            .collect();
        let response = ScoopcResponseEnvelopeV1::success(
            RequestCorrelationId::from_array([7; 16]),
            ScoopcSuccessV1::new(ScoopcProductV1::Check, vec![], vec![])
                .unwrap()
                .with_environment_inputs(inputs),
        );
        let bytes = encode_response_frame(&response).unwrap();
        assert_eq!(decode_response_frame(&bytes).unwrap(), response);
    }
}
