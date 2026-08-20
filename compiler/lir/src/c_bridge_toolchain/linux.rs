use std::fmt;

use scoop_wire::{Digest256, Encoder, WireEncode};

/// Content of the selected compiler, wrapper/specs and consumed C headers.
/// Paths are retained by tool discovery, not by the artifact contract.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct GccCompilerIdentityV1 {
    version: String,
    inputs: Digest256,
}

impl GccCompilerIdentityV1 {
    pub fn new(version: &str, inputs: Digest256) -> Result<Self, GccCompilerIdentityError> {
        let components: Vec<_> = version.split('.').collect();
        if components.is_empty()
            || components.len() > 3
            || components.iter().any(|component| {
                !component.bytes().all(|byte| byte.is_ascii_digit())
                    || component.parse::<u32>().is_err()
            })
            || components[0].parse::<u32>().ok() == Some(0)
        {
            return Err(GccCompilerIdentityError);
        }
        Ok(Self {
            version: version.to_owned(),
            inputs,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }
    pub const fn inputs(&self) -> Digest256 {
        self.inputs
    }
}

impl WireEncode for GccCompilerIdentityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.unsigned(2)?;
        encoder.field(2)?;
        encoder.text(&self.version)?;
        encoder.field(3)?;
        self.inputs.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GccCompilerIdentityError;

impl fmt::Display for GccCompilerIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GCC must report a nonzero numeric version")
    }
}

impl std::error::Error for GccCompilerIdentityError {}
