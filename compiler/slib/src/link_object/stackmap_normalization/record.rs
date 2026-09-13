use std::fmt;

use scoop_identity::{PersistentCallableBodyId, PersistentSafepointSiteId, SafepointSiteRole};
use scoop_wire::{RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, WireEncode};

const STACKMAP_FORMAT_VERSION: u32 = 3;

/// One raw LLVM v3 location before ConstantIndex normalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProvisionalLlvmStackmapLocationV3 {
    pub(super) kind: u8,
    pub(super) size: u16,
    pub(super) dwarf_register: u16,
    pub(super) offset: i32,
}

impl ProvisionalLlvmStackmapLocationV3 {
    pub const fn new(kind: u8, size: u16, dwarf_register: u16, offset: i32) -> Self {
        Self {
            kind,
            size,
            dwarf_register,
            offset,
        }
    }

    pub const fn kind(self) -> u8 {
        self.kind
    }

    pub const fn size(self) -> u16 {
        self.size
    }

    pub const fn dwarf_register(self) -> u16 {
        self.dwarf_register
    }

    pub const fn offset(self) -> i32 {
        self.offset
    }
}

/// One raw LLVM v3 live-out entry after reserved-byte validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProvisionalLlvmStackmapLiveOutV3 {
    pub(super) dwarf_register: u16,
    pub(super) size: u8,
}

impl ProvisionalLlvmStackmapLiveOutV3 {
    pub const fn new(dwarf_register: u16, size: u8) -> Self {
        Self {
            dwarf_register,
            size,
        }
    }

    pub const fn dwarf_register(self) -> u16 {
        self.dwarf_register
    }

    pub const fn size(self) -> u8 {
        self.size
    }
}

/// Fixed LLVM v3 record fields plus the function owner resolved from relocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProvisionalLlvmStackmapRecordHeaderV3 {
    pub(super) safepoint_id: u64,
    pub(super) owner: PersistentCallableBodyId,
    pub(super) instruction_offset: u32,
    pub(super) stack_size: u64,
    pub(super) flags: u16,
}

impl ProvisionalLlvmStackmapRecordHeaderV3 {
    pub const fn new(
        safepoint_id: u64,
        owner: PersistentCallableBodyId,
        instruction_offset: u32,
        stack_size: u64,
        flags: u16,
    ) -> Self {
        Self {
            safepoint_id,
            owner,
            instruction_offset,
            stack_size,
            flags,
        }
    }
}

/// Parsed physical record plus its function owner and constant pool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvisionalLlvmStackmapRecordV3 {
    pub(super) header: ProvisionalLlvmStackmapRecordHeaderV3,
    pub(super) constant_pool: Vec<u64>,
    pub(super) locations: Vec<ProvisionalLlvmStackmapLocationV3>,
    pub(super) live_outs: Vec<ProvisionalLlvmStackmapLiveOutV3>,
}

impl ProvisionalLlvmStackmapRecordV3 {
    pub fn new(
        header: ProvisionalLlvmStackmapRecordHeaderV3,
        constant_pool: Vec<u64>,
        locations: Vec<ProvisionalLlvmStackmapLocationV3>,
        live_outs: Vec<ProvisionalLlvmStackmapLiveOutV3>,
    ) -> Self {
        Self {
            header,
            constant_pool,
            locations,
            live_outs,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalStackmapLocationV1 {
    Register {
        size: u32,
        dwarf_register: u32,
    },
    Direct {
        size: u32,
        dwarf_register: u32,
        signed_offset_bits: u64,
    },
    Indirect {
        size: u32,
        dwarf_register: u32,
        signed_offset_bits: u64,
    },
    Constant {
        size: u32,
        value_bits: u64,
    },
}

impl RuntimeEncode for CanonicalStackmapLocationV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        match self {
            Self::Register {
                size,
                dwarf_register,
            } => {
                encoder.u32(1)?;
                encoder.u32(*size)?;
                encoder.u32(*dwarf_register)
            }
            Self::Direct {
                size,
                dwarf_register,
                signed_offset_bits,
            } => {
                encoder.u32(2)?;
                encoder.u32(*size)?;
                encoder.u32(*dwarf_register)?;
                encoder.u64(*signed_offset_bits)
            }
            Self::Indirect {
                size,
                dwarf_register,
                signed_offset_bits,
            } => {
                encoder.u32(3)?;
                encoder.u32(*size)?;
                encoder.u32(*dwarf_register)?;
                encoder.u64(*signed_offset_bits)
            }
            Self::Constant { size, value_bits } => {
                encoder.u32(4)?;
                encoder.u32(*size)?;
                encoder.u64(*value_bits)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalStackmapLiveOutV1 {
    pub(super) dwarf_register: u32,
    pub(super) size: u32,
}

impl CanonicalStackmapLiveOutV1 {
    pub const fn dwarf_register(self) -> u32 {
        self.dwarf_register
    }

    pub const fn size(self) -> u32 {
        self.size
    }
}

impl RuntimeEncode for CanonicalStackmapLiveOutV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(self.dwarf_register)?;
        encoder.u32(self.size)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalStackmapRecordV1 {
    pub(super) site: PersistentSafepointSiteId,
    pub(super) safepoint_id: u64,
    pub(super) owner: PersistentCallableBodyId,
    pub(super) role: SafepointSiteRole,
    pub(super) root_pair_count: u32,
    pub(super) instruction_offset: u32,
    pub(super) stack_size: u64,
    pub(super) locations: Vec<CanonicalStackmapLocationV1>,
    pub(super) live_outs: Vec<CanonicalStackmapLiveOutV1>,
}

impl CanonicalStackmapRecordV1 {
    pub const fn format_version(&self) -> u32 {
        STACKMAP_FORMAT_VERSION
    }

    pub const fn site(&self) -> PersistentSafepointSiteId {
        self.site
    }

    pub const fn safepoint_id(&self) -> u64 {
        self.safepoint_id
    }

    pub const fn owner(&self) -> PersistentCallableBodyId {
        self.owner
    }

    pub const fn role(&self) -> SafepointSiteRole {
        self.role
    }

    pub const fn root_pair_count(&self) -> u32 {
        self.root_pair_count
    }

    pub const fn instruction_offset(&self) -> u32 {
        self.instruction_offset
    }

    pub const fn stack_size(&self) -> u64 {
        self.stack_size
    }

    pub fn locations(&self) -> &[CanonicalStackmapLocationV1] {
        &self.locations
    }

    pub fn live_outs(&self) -> &[CanonicalStackmapLiveOutV1] {
        &self.live_outs
    }
}

impl RuntimeEncode for CanonicalStackmapRecordV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(STACKMAP_FORMAT_VERSION)?;
        self.site.runtime_encode(encoder)?;
        encoder.u64(self.safepoint_id)?;
        self.owner.runtime_encode(encoder)?;
        self.role.runtime_encode(encoder)?;
        encoder.u32(self.root_pair_count)?;
        encoder.u32(self.instruction_offset)?;
        encoder.u64(self.stack_size)?;
        encoder.sequence_length(self.locations.len())?;
        for location in &self.locations {
            location.runtime_encode(encoder)?;
        }
        encoder.sequence_length(self.live_outs.len())?;
        for live_out in &self.live_outs {
            live_out.runtime_encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StackmapRecordFingerprintV1(pub(super) [u8; 32]);

impl StackmapRecordFingerprintV1 {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for StackmapRecordFingerprintV1 {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl RuntimeEncode for StackmapRecordFingerprintV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.fixed(&self.0)
    }
}

impl fmt::Display for StackmapRecordFingerprintV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedNormalizedStackmapRecordV1 {
    pub(super) canonical: CanonicalStackmapRecordV1,
    pub(super) fingerprint: StackmapRecordFingerprintV1,
}

impl VerifiedNormalizedStackmapRecordV1 {
    pub const fn canonical(&self) -> &CanonicalStackmapRecordV1 {
        &self.canonical
    }

    pub const fn fingerprint(&self) -> StackmapRecordFingerprintV1 {
        self.fingerprint
    }
}
