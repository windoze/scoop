//! Observed compiler inputs used only by source-build update decisions.

use crate::{EnvironmentInputV1, HostPathCarrier};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

mod directories;
pub use directories::{DirectoryEntryInputV1, DirectoryEntryStateV1, DirectoryInputV1};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompilerInputV1 {
    Environment(EnvironmentInputV1),
    File(FileInputV1),
    Directory(DirectoryInputV1),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FileInputV1 {
    pub root: HostPathCarrier,
    pub real_root: HostPathCarrier,
    pub locator: HostPathCarrier,
    pub resolved: HostPathCarrier,
    pub length: u64,
    pub modified: (bool, u64, u32),
}

impl WireEncode for CompilerInputV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(2)?;
        match self {
            Self::Environment(input) => {
                e.unsigned(1)?;
                input.encode(e)
            }
            Self::File(input) => {
                e.unsigned(2)?;
                input.encode(e)
            }
            Self::Directory(input) => {
                e.unsigned(3)?;
                input.encode(e)
            }
        }
    }
}

impl WireDecode for CompilerInputV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        let length = d.array()?;
        if length != 2 {
            return Err(invalid(d));
        }
        match d.unsigned()? {
            1 => EnvironmentInputV1::decode(d).map(Self::Environment),
            2 => FileInputV1::decode(d).map(Self::File),
            3 => DirectoryInputV1::decode(d).map(Self::Directory),
            _ => Err(invalid(d)),
        }
    }
}

impl WireEncode for FileInputV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(8)?;
        for path in [&self.root, &self.real_root, &self.locator, &self.resolved] {
            path.encode(e)?;
        }
        e.unsigned(self.length)?;
        e.unsigned(u64::from(self.modified.0))?;
        e.unsigned(self.modified.1)?;
        e.unsigned(u64::from(self.modified.2))
    }
}

impl WireDecode for FileInputV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        if d.array()? != 8 {
            return Err(invalid(d));
        }
        let root = decode_path(d)?;
        let real_root = decode_path(d)?;
        let locator = decode_path(d)?;
        let resolved = decode_path(d)?;
        let length = d.unsigned()?;
        let modified = decode_modified(d)?;
        Ok(Self {
            root,
            real_root,
            locator,
            resolved,
            length,
            modified,
        })
    }
}

fn decode_path(d: &mut Decoder<'_>) -> Result<HostPathCarrier, WireError> {
    crate::DecodedHostPathCarrier::decode(d)?
        .validate()
        .map_err(|_| invalid(d))
}

fn decode_modified(d: &mut Decoder<'_>) -> Result<(bool, u64, u32), WireError> {
    let negative = d.unsigned()?;
    let seconds = d.unsigned()?;
    let nanos = d.unsigned()?;
    if negative > 1 || nanos >= 1_000_000_000 {
        return Err(invalid(d));
    }
    Ok((negative == 1, seconds, nanos as u32))
}

fn invalid(d: &Decoder<'_>) -> WireError {
    WireError::new(
        scoop_wire::WireErrorKind::IntegerOutOfRange,
        d.path().clone(),
        Some(d.position()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn include_file_input_response_roundtrip_preserves_original_locator() {
        let path = |text| HostPathCarrier::from_path(std::path::Path::new(text)).unwrap();
        let input = CompilerInputV1::File(FileInputV1 {
            root: path("/alias"),
            real_root: path("/real"),
            locator: path("/real/src/link/../data"),
            resolved: path("/real/data"),
            length: 123,
            modified: (true, 100, 42),
        });
        let response = crate::ScoopcResponseEnvelopeV1::success(
            crate::RequestCorrelationId::from_array([8; 16]),
            crate::ScoopcSuccessV1::new(crate::ScoopcProductV1::Check, vec![], vec![])
                .unwrap()
                .with_compiler_inputs(vec![input]),
        );
        let bytes = crate::encode_response_frame(&response).unwrap();
        assert_eq!(crate::decode_response_frame(&bytes).unwrap(), response);
    }
}
