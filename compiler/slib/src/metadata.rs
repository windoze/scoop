use std::fmt;

use scoop_identity::{CapabilityId, CapabilityIdError, DecodedCapabilityId};
use scoop_wire::{
    BorrowedWireDecode, BudgetMeter, DecodeLimits, Decoder, Encoder, WireDecode, WireEncode,
    WireError, decode_canonical_borrowed_with_meter,
};

use crate::{CapabilityContractRegistry, MemberPurposeSet, SectionLocation};

const INITIAL_SCHEMA: u32 = 1;
const MAX_SECTION_PAYLOAD_BYTES: u64 = 268_435_456;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MetadataLocation {
    Hir,
    Mir,
    Lir,
}

impl MetadataLocation {
    const fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::Hir => b"SCOOPHIR",
            Self::Mir => b"SCOOPMIR",
            Self::Lir => b"SCOOPLIR",
        }
    }
}

impl fmt::Display for MetadataLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Hir => "HIR",
            Self::Mir => "MIR",
            Self::Lir => "LIR",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataSection {
    location: MetadataLocation,
    capability: CapabilityId,
    required_for: MemberPurposeSet,
    payload: Vec<u8>,
}

impl MetadataSection {
    pub fn new(
        location: MetadataLocation,
        capability: CapabilityId,
        required_for: MemberPurposeSet,
        payload: Vec<u8>,
    ) -> Result<Self, MetadataSectionError> {
        validate_section(location, &capability, required_for, payload.len())?;
        Ok(Self {
            location,
            capability,
            required_for,
            payload,
        })
    }

    pub const fn location(&self) -> MetadataLocation {
        self.location
    }

    pub const fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub const fn required_for(&self) -> MemberPurposeSet {
        self.required_for
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl WireEncode for MetadataSection {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.capability.encode(encoder)?;
        encoder.field(2)?;
        self.required_for.encode(encoder)?;
        encoder.field(3)?;
        encoder.bytes(&self.payload)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataEnvelope {
    location: MetadataLocation,
    sections: Vec<MetadataSection>,
}

impl MetadataEnvelope {
    pub fn new(
        location: MetadataLocation,
        mut sections: Vec<MetadataSection>,
    ) -> Result<Self, MetadataEnvelopeError> {
        for (index, section) in sections.iter().enumerate() {
            if section.location != location {
                return Err(MetadataEnvelopeError::SectionLocationMismatch {
                    index,
                    envelope: location,
                    section: section.location,
                });
            }
        }
        sections.sort_unstable_by(|left, right| left.capability.cmp(&right.capability));
        if let Some(pair) = sections
            .windows(2)
            .find(|pair| pair[0].capability == pair[1].capability)
        {
            return Err(MetadataEnvelopeError::DuplicateSection {
                capability: pair[0].capability.clone(),
            });
        }
        Ok(Self { location, sections })
    }

    pub const fn location(&self) -> MetadataLocation {
        self.location
    }

    pub fn sections(&self) -> &[MetadataSection] {
        &self.sections
    }
}

impl WireEncode for MetadataEnvelope {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.bytes(self.location.magic())?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(INITIAL_SCHEMA))?;
        encoder.field(3)?;
        encoder.array(self.sections.len() as u64)?;
        for section in &self.sections {
            section.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedMetadataEnvelope<'input> {
    location: MetadataLocation,
    sections: Vec<DecodedMetadataSection<'input>>,
}

impl<'input> DecodedMetadataEnvelope<'input> {
    pub fn decode(
        input: &'input [u8],
        location: MetadataLocation,
        limits: DecodeLimits,
    ) -> Result<Self, MetadataReadError> {
        let mut meter = BudgetMeter::new(limits);
        Self::decode_with_meter(input, location, &mut meter)
    }

    pub fn decode_with_meter(
        input: &'input [u8],
        location: MetadataLocation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MetadataReadError> {
        let decoded =
            decode_canonical_borrowed_with_meter::<UnvalidatedMetadataEnvelope<'_>>(input, meter)
                .map_err(MetadataReadError::CanonicalWire)?;
        decoded.validate(location, meter)
    }

    pub const fn location(&self) -> MetadataLocation {
        self.location
    }

    pub fn sections(&self) -> &[DecodedMetadataSection<'input>] {
        &self.sections
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedMetadataSection<'input> {
    location: MetadataLocation,
    capability: CapabilityId,
    required_for: MemberPurposeSet,
    payload: &'input [u8],
}

impl<'input> DecodedMetadataSection<'input> {
    pub const fn location(&self) -> MetadataLocation {
        self.location
    }

    pub const fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub const fn required_for(&self) -> MemberPurposeSet {
        self.required_for
    }

    pub const fn payload(&self) -> &'input [u8] {
        self.payload
    }
}

struct UnvalidatedMetadataEnvelope<'input> {
    magic: &'input [u8],
    outer_schema: u32,
    sections: Vec<UnvalidatedMetadataSection<'input>>,
}

impl<'input> UnvalidatedMetadataEnvelope<'input> {
    fn validate(
        self,
        location: MetadataLocation,
        meter: &mut BudgetMeter,
    ) -> Result<DecodedMetadataEnvelope<'input>, MetadataReadError> {
        if self.magic != location.magic() {
            return Err(MetadataReadError::BadMagic { expected: location });
        }
        if self.outer_schema != INITIAL_SCHEMA {
            return Err(MetadataReadError::UnsupportedSchema {
                actual: self.outer_schema,
            });
        }
        meter
            .charge_canonical_sequence(self.sections.len() as u64, &Default::default())
            .map_err(MetadataReadError::Budget)?;
        meter
            .charge_collection_slots(self.sections.len() as u64, &Default::default())
            .map_err(MetadataReadError::Budget)?;

        let mut sections = Vec::new();
        sections
            .try_reserve_exact(self.sections.len())
            .map_err(|_| MetadataReadError::Allocation)?;
        for (index, section) in self.sections.into_iter().enumerate() {
            meter
                .charge_work(32, &Default::default())
                .map_err(MetadataReadError::Budget)?;
            sections.push(
                section
                    .validate(location)
                    .map_err(|error| MetadataReadError::Section { index, error })?,
            );
        }
        if let Some((index, _)) = sections
            .windows(2)
            .enumerate()
            .find(|(_, pair)| pair[0].capability >= pair[1].capability)
        {
            return Err(MetadataReadError::NonIncreasingSection {
                first_index: index,
                second_index: index + 1,
            });
        }
        Ok(DecodedMetadataEnvelope { location, sections })
    }
}

impl<'input> BorrowedWireDecode<'input> for UnvalidatedMetadataEnvelope<'input> {
    fn decode(decoder: &mut Decoder<'input, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            magic: decoder.field(1, Decoder::carrier_bytes)?,
            outer_schema: decoder.field(2, Decoder::u32)?,
            sections: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| UnvalidatedMetadataSection::decode(decoder))
            })?,
        })
    }
}

struct UnvalidatedMetadataSection<'input> {
    capability: DecodedCapabilityId,
    required_for: u32,
    payload: &'input [u8],
}

impl<'input> UnvalidatedMetadataSection<'input> {
    fn decode(decoder: &mut Decoder<'input, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            capability: decoder.field(1, DecodedCapabilityId::decode)?,
            required_for: decoder.field(2, Decoder::u32)?,
            payload: decoder.field(3, Decoder::carrier_bytes)?,
        })
    }

    fn validate(
        self,
        location: MetadataLocation,
    ) -> Result<DecodedMetadataSection<'input>, MetadataSectionValidationError> {
        let capability = self
            .capability
            .validate()
            .map_err(MetadataSectionValidationError::Capability)?;
        let required_for = MemberPurposeSet::from_bits(self.required_for);
        validate_section(location, &capability, required_for, self.payload.len())
            .map_err(MetadataSectionValidationError::Section)?;
        Ok(DecodedMetadataSection {
            location,
            capability,
            required_for,
            payload: self.payload,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataSectionError {
    InvalidPurpose {
        location: MetadataLocation,
        bits: u32,
    },
    KnownCapabilityWrongLocation {
        capability: CapabilityId,
        expected: SectionLocation,
        actual: SectionLocation,
    },
    KnownCapabilityWrongPurpose {
        capability: CapabilityId,
        expected: MemberPurposeSet,
        bits: u32,
    },
    PayloadTooLarge {
        actual: u64,
    },
}

impl fmt::Display for MetadataSectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPurpose { location, bits } => {
                write!(
                    formatter,
                    "invalid {location} metadata purpose bits {bits:#x}"
                )
            }
            Self::KnownCapabilityWrongLocation {
                capability,
                expected,
                actual,
            } => write!(
                formatter,
                "capability {}/{}/{} belongs in {expected}, not {actual}",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
            ),
            Self::KnownCapabilityWrongPurpose {
                capability,
                expected,
                bits,
            } => write!(
                formatter,
                "capability {}/{}/{} requires purpose bits {:#x}, found {bits:#x}",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
                expected.bits(),
            ),
            Self::PayloadTooLarge { actual } => write!(
                formatter,
                "metadata section payload exceeds 268435456 bytes: found {actual}"
            ),
        }
    }
}

impl std::error::Error for MetadataSectionError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataEnvelopeError {
    SectionLocationMismatch {
        index: usize,
        envelope: MetadataLocation,
        section: MetadataLocation,
    },
    DuplicateSection {
        capability: CapabilityId,
    },
}

impl fmt::Display for MetadataEnvelopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SectionLocationMismatch {
                index,
                envelope,
                section,
            } => write!(
                formatter,
                "metadata section {index} was built for {section}, not envelope {envelope}"
            ),
            Self::DuplicateSection { capability } => write!(
                formatter,
                "duplicate metadata section capability {}/{}/{}",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
            ),
        }
    }
}

impl std::error::Error for MetadataEnvelopeError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataSectionValidationError {
    Capability(CapabilityIdError),
    Section(MetadataSectionError),
}

impl fmt::Display for MetadataSectionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capability(error) => error.fmt(formatter),
            Self::Section(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for MetadataSectionValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataReadError {
    CanonicalWire(WireError),
    Budget(WireError),
    BadMagic {
        expected: MetadataLocation,
    },
    UnsupportedSchema {
        actual: u32,
    },
    Section {
        index: usize,
        error: MetadataSectionValidationError,
    },
    NonIncreasingSection {
        first_index: usize,
        second_index: usize,
    },
    Allocation,
}

impl fmt::Display for MetadataReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CanonicalWire(error) | Self::Budget(error) => error.fmt(formatter),
            Self::BadMagic { expected } => {
                write!(formatter, "metadata magic does not match {expected}")
            }
            Self::UnsupportedSchema { actual } => {
                write!(formatter, "metadata outer schema must be 1, found {actual}")
            }
            Self::Section { index, error } => {
                write!(formatter, "invalid metadata section {index}: {error}")
            }
            Self::NonIncreasingSection {
                first_index,
                second_index,
            } => write!(
                formatter,
                "metadata section capabilities at indexes {first_index} and {second_index} are not strictly increasing"
            ),
            Self::Allocation => formatter.write_str("failed to allocate metadata section table"),
        }
    }
}

impl std::error::Error for MetadataReadError {}

fn validate_section(
    location: MetadataLocation,
    capability: &CapabilityId,
    required_for: MemberPurposeSet,
    payload_length: usize,
) -> Result<(), MetadataSectionError> {
    let actual = u64::try_from(payload_length).unwrap_or(u64::MAX);
    if actual > MAX_SECTION_PAYLOAD_BYTES {
        return Err(MetadataSectionError::PayloadTooLarge { actual });
    }
    let bits = required_for.bits();
    let purpose_valid = match location {
        MetadataLocation::Hir | MetadataLocation::Mir => matches!(bits, 0 | 2),
        MetadataLocation::Lir => matches!(bits, 0 | 2 | 4 | 6),
    };
    if !purpose_valid {
        return Err(MetadataSectionError::InvalidPurpose { location, bits });
    }
    if let Some(contract) = CapabilityContractRegistry::contract(capability) {
        let actual = section_location(location);
        if contract.location() != actual {
            return Err(MetadataSectionError::KnownCapabilityWrongLocation {
                capability: capability.clone(),
                expected: contract.location(),
                actual,
            });
        }
        if required_for != contract.required_for() {
            return Err(MetadataSectionError::KnownCapabilityWrongPurpose {
                capability: capability.clone(),
                expected: contract.required_for(),
                bits,
            });
        }
    }
    Ok(())
}

const fn section_location(location: MetadataLocation) -> SectionLocation {
    match location {
        MetadataLocation::Hir => SectionLocation::Hir,
        MetadataLocation::Mir => SectionLocation::Mir,
        MetadataLocation::Lir => SectionLocation::Lir,
    }
}

#[cfg(test)]
mod tests;
