//! Native format facts shared by builtin object consumers.

use std::num::NonZeroU32;

use scoop_identity::TargetProfileId;
use scoop_wire::Digest256;

use super::{DarwinArm64RelocationShapeV1, DarwinDeploymentCommandV1};

mod macho;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectEnvelopeFormatV1 {
    DarwinArm64 {
        load_command_count: u32,
        deployment: Option<DarwinDeploymentCommandV1>,
    },
    Elf64 {
        target: TargetProfileId,
        flags: u32,
        groups: Vec<ObservedElfGroupV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedElfGroupV1 {
    pub signature_symbol: u32,
    pub flags: u32,
    pub sections: Vec<NonZeroU32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedObjectEnvelopeV1 {
    pub(super) byte_length: u64,
    pub(super) content_digest: Digest256,
    pub(super) format: ObjectEnvelopeFormatV1,
    pub(super) sections: Vec<ObservedObjectSectionV1>,
    pub(super) symbols: Vec<ObservedObjectSymbolV1>,
    pub(super) relocations: Vec<ObservedObjectRelocationV1>,
}

impl ValidatedObjectEnvelopeV1 {
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
    pub const fn content_digest(&self) -> Digest256 {
        self.content_digest
    }
    pub const fn format(&self) -> &ObjectEnvelopeFormatV1 {
        &self.format
    }
    pub const fn target(&self) -> TargetProfileId {
        match self.format {
            ObjectEnvelopeFormatV1::DarwinArm64 { .. } => TargetProfileId::DarwinAarch64,
            ObjectEnvelopeFormatV1::Elf64 { target, .. } => target,
        }
    }
    pub fn sections(&self) -> &[ObservedObjectSectionV1] {
        &self.sections
    }
    pub fn symbols(&self) -> &[ObservedObjectSymbolV1] {
        &self.symbols
    }
    pub fn relocations(&self) -> &[ObservedObjectRelocationV1] {
        &self.relocations
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectSectionFlagsV1 {
    MachO(u32),
    Elf { section_type: u32, flags: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedObjectSectionV1 {
    pub(super) segment_name: Vec<u8>,
    pub(super) section_name: Vec<u8>,
    pub(super) virtual_address: u64,
    pub(super) file_offset: Option<u64>,
    pub(super) flags: ObjectSectionFlagsV1,
    pub(super) byte_size: u64,
    pub(super) alignment_power: u32,
}

impl ObservedObjectSectionV1 {
    pub fn segment_name(&self) -> &[u8] {
        &self.segment_name
    }
    pub fn section_name(&self) -> &[u8] {
        &self.section_name
    }
    pub const fn flags(&self) -> ObjectSectionFlagsV1 {
        self.flags
    }
    pub const fn virtual_address(&self) -> u64 {
        self.virtual_address
    }
    pub(crate) const fn file_offset(&self) -> Option<u64> {
        self.file_offset
    }
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }
    pub const fn alignment_power(&self) -> u32 {
        self.alignment_power
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectSymbolKindV1 {
    LocalSectionDefinition,
    ExternalStrongDefinition,
    ExternalWeakDefinition,
    ExternalUndefined,
    SectionBase,
    FileMetadata,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectSymbolAttributesV1 {
    MachO {
        no_dead_strip: bool,
        private_external: bool,
    },
    Elf {
        size: u64,
        info: u8,
        other: u8,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedObjectSymbolV1 {
    pub(super) table_index: u32,
    pub(super) name: Vec<u8>,
    pub(super) kind: ObjectSymbolKindV1,
    pub(super) section_ordinal: Option<NonZeroU32>,
    pub(super) value: u64,
    pub(super) attributes: ObjectSymbolAttributesV1,
}

impl ObservedObjectSymbolV1 {
    pub const fn table_index(&self) -> u32 {
        self.table_index
    }
    pub fn name(&self) -> &[u8] {
        &self.name
    }
    pub const fn kind(&self) -> ObjectSymbolKindV1 {
        self.kind
    }
    pub const fn section_ordinal(&self) -> Option<NonZeroU32> {
        self.section_ordinal
    }
    pub const fn value(&self) -> u64 {
        self.value
    }
    pub const fn attributes(&self) -> ObjectSymbolAttributesV1 {
        self.attributes
    }
    pub const fn no_dead_strip(&self) -> bool {
        matches!(
            self.attributes,
            ObjectSymbolAttributesV1::MachO {
                no_dead_strip: true,
                ..
            }
        )
    }
    pub const fn private_external(&self) -> bool {
        match self.attributes {
            ObjectSymbolAttributesV1::MachO {
                private_external, ..
            } => private_external,
            ObjectSymbolAttributesV1::Elf { other, .. } => other & 3 == object::elf::STV_HIDDEN,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectRelocationShapeV1 {
    DarwinArm64(DarwinArm64RelocationShapeV1),
    ElfRela {
        kind: u32,
        target_symbol: u32,
        addend: i64,
        width: u8,
    },
}

impl ObjectRelocationShapeV1 {
    pub const fn width_bytes(self) -> u8 {
        match self {
            Self::DarwinArm64(shape) => shape.width_bytes(),
            Self::ElfRela { width, .. } => width,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedObjectRelocationV1 {
    pub(super) containing_section_ordinal: NonZeroU32,
    pub(super) offset: u64,
    pub(super) encoded_value: u64,
    pub(super) shape: ObjectRelocationShapeV1,
}

impl ObservedObjectRelocationV1 {
    pub const fn containing_section_ordinal(&self) -> NonZeroU32 {
        self.containing_section_ordinal
    }
    pub const fn offset(&self) -> u64 {
        self.offset
    }
    pub const fn encoded_value(&self) -> u64 {
        self.encoded_value
    }
    pub const fn shape(&self) -> ObjectRelocationShapeV1 {
        self.shape
    }
}
