use crate::{DecodedHostPathCarrier, HostPathCarrier, ProtocolValidationError};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A host program selected explicitly by the receiving CLI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratorRegistrationV1 {
    name: String,
    program: HostPathCarrier,
}

impl GeneratorRegistrationV1 {
    pub fn new(name: String, program: HostPathCarrier) -> Result<Self, ProtocolValidationError> {
        if name.trim().is_empty() {
            return Err(ProtocolValidationError::InvalidGeneratorRegistration);
        }
        Ok(Self { name, program })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn program(&self) -> &HostPathCarrier {
        &self.program
    }
}

impl WireEncode for GeneratorRegistrationV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(2)?;
        e.field(1)?;
        e.text(&self.name)?;
        e.field(2)?;
        self.program.encode(e)
    }
}

impl WireDecode for GeneratorRegistrationV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(2)?;
        let name = d.field(1, Decoder::owned_text)?;
        let program = d.field(2, DecodedHostPathCarrier::decode)?;
        let program = program.validate().map_err(|_| invalid(d))?;
        Self::new(name, program).map_err(|_| invalid(d))
    }
}

/// Update inputs captured for a program that actually ran in this compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratorToolInputV1 {
    registration: GeneratorRegistrationV1,
    protocol_version: u32,
    byte_length: u64,
    modified: SystemTime,
}

impl GeneratorToolInputV1 {
    pub fn new(
        registration: GeneratorRegistrationV1,
        protocol_version: u32,
        byte_length: u64,
        modified: SystemTime,
    ) -> Self {
        Self {
            registration,
            protocol_version,
            byte_length,
            modified,
        }
    }

    pub fn registration(&self) -> &GeneratorRegistrationV1 {
        &self.registration
    }
    pub const fn protocol_version(&self) -> u32 {
        self.protocol_version
    }
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
    pub const fn modified(&self) -> SystemTime {
        self.modified
    }
}

impl WireEncode for GeneratorToolInputV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(4)?;
        e.field(1)?;
        self.registration.encode(e)?;
        e.field(2)?;
        e.unsigned(u64::from(self.protocol_version))?;
        e.field(3)?;
        e.unsigned(self.byte_length)?;
        e.field(4)?;
        let (negative, duration) = match self.modified.duration_since(UNIX_EPOCH) {
            Ok(duration) => (0, duration),
            Err(error) => (1, error.duration()),
        };
        e.array(3)?;
        e.unsigned(negative)?;
        e.unsigned(duration.as_secs())?;
        e.unsigned(u64::from(duration.subsec_nanos()))
    }
}

impl WireDecode for GeneratorToolInputV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(4)?;
        let registration = d.field(1, GeneratorRegistrationV1::decode)?;
        let protocol_version =
            d.field(2, |d| u32::try_from(d.unsigned()?).map_err(|_| invalid(d)))?;
        let byte_length = d.field(3, Decoder::unsigned)?;
        let modified = d.field(4, |d| {
            if d.array()? != 3 {
                return Err(invalid(d));
            }
            let negative = d.unsigned()?;
            let seconds = d.unsigned()?;
            let nanos = d.unsigned()?;
            if nanos >= 1_000_000_000 {
                return Err(invalid(d));
            }
            let duration = Duration::new(seconds, nanos as u32);
            match negative {
                0 => UNIX_EPOCH.checked_add(duration),
                1 => UNIX_EPOCH.checked_sub(duration),
                _ => None,
            }
            .ok_or_else(|| invalid(d))
        })?;
        Ok(Self::new(
            registration,
            protocol_version,
            byte_length,
            modified,
        ))
    }
}

fn invalid(d: &Decoder<'_>) -> WireError {
    WireError::new(
        scoop_wire::WireErrorKind::UnknownTag { tag: u64::MAX },
        d.path().clone(),
        Some(d.position()),
    )
}

#[cfg(test)]
mod tests;
