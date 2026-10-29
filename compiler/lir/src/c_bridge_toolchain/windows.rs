use scoop_wire::{Digest256, Encoder, WireEncode};

/// Clang and header configuration used by the Windows generated-C producer.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WindowsClangCompilerIdentityV1 {
    version: [u32; 3],
    inputs: Digest256,
}

impl WindowsClangCompilerIdentityV1 {
    pub const fn new(version: [u32; 3], inputs: Digest256) -> Self {
        Self { version, inputs }
    }

    pub const fn version(&self) -> [u32; 3] {
        self.version
    }

    pub const fn inputs(&self) -> Digest256 {
        self.inputs
    }
}

impl WireEncode for WindowsClangCompilerIdentityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.unsigned(3)?;
        encoder.field(2)?;
        encoder.array(3)?;
        for value in self.version {
            encoder.unsigned(u64::from(value))?;
        }
        encoder.field(3)?;
        self.inputs.encode(encoder)
    }
}
