use super::*;
use scoop_wire::cbor::EncodeError;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl WireEncode for TestDirectory {
    fn encode(&self, e: &mut Encoder) -> Result<(), EncodeError> {
        e.array(self.containers.len() as u64)?;
        for container in &self.containers {
            e.map(3)?;
            e.field(1)?;
            e.text(&container.qualified_name)?;
            e.field(2)?;
            container.location.encode(e)?;
            e.field(3)?;
            e.array(container.cases.len() as u64)?;
            for case in &container.cases {
                e.map(4)?;
                e.field(1)?;
                e.text(&case.name)?;
                e.field(2)?;
                e.text(&case.display_name)?;
                e.field(3)?;
                case.location.encode(e)?;
                e.field(4)?;
                e.array(u64::from(case.ignore_reason.is_some()))?;
                if let Some(reason) = &case.ignore_reason {
                    e.text(reason)?;
                }
            }
        }
        Ok(())
    }
}

impl WireEncode for TestSourceLocation {
    fn encode(&self, e: &mut Encoder) -> Result<(), EncodeError> {
        e.map(3)?;
        e.field(1)?;
        e.text(&self.path)?;
        e.field(2)?;
        e.unsigned(u64::from(self.line))?;
        e.field(3)?;
        e.unsigned(u64::from(self.column))
    }
}

impl WireDecode for TestDirectory {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        let containers = d.decode_array(|d, _| {
            d.expect_map(3)?;
            Ok(TestContainerListing {
                qualified_name: d.field(1, Decoder::owned_text)?,
                location: d.field(2, TestSourceLocation::decode)?,
                cases: d.field(3, |d| {
                    d.decode_array(|d, _| {
                        d.expect_map(4)?;
                        Ok(TestCaseListing {
                            name: d.field(1, Decoder::owned_text)?,
                            display_name: d.field(2, Decoder::owned_text)?,
                            location: d.field(3, TestSourceLocation::decode)?,
                            ignore_reason: d.field(4, |d| match d.array()? {
                                0 => Ok(None),
                                1 => d.owned_text().map(Some),
                                actual => Err(WireError::new(
                                    WireErrorKind::InvalidLength {
                                        expected: 1,
                                        actual,
                                    },
                                    d.path().clone(),
                                    Some(d.position()),
                                )),
                            })?,
                        })
                    })
                })?,
            })
        })?;
        Ok(Self { containers })
    }
}

impl WireDecode for TestSourceLocation {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(3)?;
        Ok(Self {
            path: d.field(1, Decoder::owned_text)?,
            line: d.field(2, Decoder::u32)?,
            column: d.field(3, Decoder::u32)?,
        })
    }
}
