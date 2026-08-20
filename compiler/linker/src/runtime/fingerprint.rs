use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

impl WireEncode for RuntimeBuildConfiguration {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(3)?;
        e.field(1)?;
        self.input_key.encode(e)?;
        e.field(2)?;
        self.compiler_digest.encode(e)?;
        e.field(3)?;
        e.array(self.flags.len() as u64)?;
        for flag in &self.flags {
            e.text(flag)?;
        }
        Ok(())
    }
}
impl WireDecode for RuntimeBuildConfiguration {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(3)?;
        Ok(Self {
            input_key: d.field(1, Digest256::decode)?,
            compiler_digest: d.field(2, Digest256::decode)?,
            flags: d.field(3, |d| d.decode_array(|d, _| Ok(d.text()?.to_owned())))?,
        })
    }
}

pub(super) struct ObjectKey<'a> {
    pub digest: Digest256,
    pub info: &'a NativeObjectInfo,
}
impl WireEncode for ObjectKey<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(3)?;
        e.field(1)?;
        self.digest.encode(e)?;
        self.info.encode_fields(e, 2)
    }
}

impl WireEncode for RuntimeObject {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(5)?;
        e.field(1)?;
        self.id.0.encode(e)?;
        e.field(2)?;
        e.unsigned(self.bytes.len() as u64)?;
        e.field(3)?;
        self.digest.encode(e)?;
        self.info.encode_fields(e, 4)
    }
}

pub(super) struct ArtifactKey<'a> {
    pub target: LirTargetProfile,
    pub toolchain: &'a [u8],
    pub configuration: &'a RuntimeBuildConfiguration,
    pub objects: &'a [RuntimeObject],
    pub merged: &'a NativeObjectInfo,
}
impl WireEncode for ArtifactKey<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(7)?;
        e.field(1)?;
        self.target.wire_id().encode(e)?;
        e.field(2)?;
        e.bytes(self.toolchain)?;
        e.field(3)?;
        self.configuration.encode(e)?;
        e.field(4)?;
        RuntimeAbiContract.encode(e)?;
        e.field(5)?;
        e.array(self.objects.len() as u64)?;
        for object in self.objects {
            object.encode(e)?;
        }
        self.merged.encode_fields(e, 6)
    }
}
