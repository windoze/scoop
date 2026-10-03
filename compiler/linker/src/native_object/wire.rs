use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl WireEncode for NativeObjectInfo {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(2)?;
        self.encode_fields(e, 1)
    }
}

impl NativeObjectInfo {
    pub(crate) fn encode_fields(
        &self,
        e: &mut Encoder,
        first: u32,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.field(first)?;
        e.array(self.definitions.len() as u64)?;
        for (name, definition) in &self.definitions {
            e.array(4)?;
            e.text(name)?;
            e.unsigned(match definition.kind {
                NativeSymbolKind::Function => 1,
                NativeSymbolKind::Data => 2,
                NativeSymbolKind::ThreadLocal => 3,
            })?;
            e.unsigned(u64::from(definition.read_only))?;
            e.unsigned(u64::from(definition.weak))?;
        }
        e.field(first + 1)?;
        e.array(self.requirements.len() as u64)?;
        for name in &self.requirements {
            e.text(name)?;
        }
        Ok(())
    }
}

impl WireDecode for NativeObjectInfo {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(2)?;
        Self::decode_fields(d, 1)
    }
}

impl NativeObjectInfo {
    pub(crate) fn decode_fields(d: &mut Decoder<'_>, first: u32) -> Result<Self, WireError> {
        let definitions = d.field(first, |d| {
            d.decode_array(|d, _| {
                let actual = d.array()?;
                if actual != 4 {
                    return Err(wire_error(
                        d,
                        WireErrorKind::InvalidLength {
                            expected: 4,
                            actual,
                        },
                    ));
                }
                let name = d.text()?.to_owned();
                let kind = match d.unsigned()? {
                    1 => NativeSymbolKind::Function,
                    2 => NativeSymbolKind::Data,
                    3 => NativeSymbolKind::ThreadLocal,
                    tag => return Err(wire_error(d, WireErrorKind::UnknownTag { tag })),
                };
                let read_only = boolean(d)?;
                let weak = boolean(d)?;
                Ok((
                    name,
                    NativeSymbolDefinition {
                        kind,
                        read_only,
                        weak,
                    },
                ))
            })
        })?;
        let requirements = d.field(first + 1, |d| {
            d.decode_array(|d, _| Ok(d.text()?.to_owned()))
        })?;
        Ok(Self {
            definitions: definitions.into_iter().collect(),
            requirements: requirements.into_iter().collect(),
        })
    }
}

fn boolean(d: &mut Decoder<'_>) -> Result<bool, WireError> {
    match d.unsigned()? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(wire_error(d, WireErrorKind::UnknownTag { tag })),
    }
}

fn wire_error(d: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, d.path().clone(), Some(d.position()))
}
