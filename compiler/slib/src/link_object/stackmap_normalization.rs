//! Canonical LLVM v3 stackmap record normalization and fingerprinting.

use std::fmt;

use scoop_identity::{PersistentCallableBodyId, PersistentSafepointSiteId, SafepointSiteRole};
use scoop_lir::StrongSafepointSemanticPlanV1;
use scoop_wire::{HashError, domain_separated_runtime_hash};

const STACKMAP_FINGERPRINT_DOMAIN: &str = "scoop-stackmap-record-v1";
const LOCATION_REGISTER: u8 = 1;
const LOCATION_DIRECT: u8 = 2;
const LOCATION_INDIRECT: u8 = 3;
const LOCATION_CONSTANT: u8 = 4;
const LOCATION_CONSTANT_INDEX: u8 = 5;
const AARCH64_DWARF_FP: u16 = 29;
const AARCH64_DWARF_SP: u16 = 31;

mod record;
pub use record::*;

mod parser;
pub use parser::*;

mod macho;
pub use macho::*;

pub(crate) mod verification;
pub use verification::*;

pub fn normalize_darwin_aarch64_stackmap_record_v1(
    plan: StrongSafepointSemanticPlanV1,
    constant_pool: &[u64],
    provisional: ProvisionalLlvmStackmapRecordV3,
) -> Result<VerifiedNormalizedStackmapRecordV1, StackmapNormalizationError> {
    normalize_record(
        ExpectedStackmapSemanticsV1 {
            site: plan.site(),
            safepoint_id: plan.safepoint().get(),
            owner: plan.owner(),
            role: plan.role(),
            root_pair_count: plan.root_pair_count(),
        },
        constant_pool,
        provisional,
    )
}

#[derive(Clone, Copy)]
struct ExpectedStackmapSemanticsV1 {
    site: PersistentSafepointSiteId,
    safepoint_id: u64,
    owner: PersistentCallableBodyId,
    role: SafepointSiteRole,
    root_pair_count: u32,
}

fn normalize_record(
    expected: ExpectedStackmapSemanticsV1,
    constant_pool: &[u64],
    provisional: ProvisionalLlvmStackmapRecordV3,
) -> Result<VerifiedNormalizedStackmapRecordV1, StackmapNormalizationError> {
    if provisional.header.safepoint_id != expected.safepoint_id {
        return Err(StackmapNormalizationError::SafepointIdMismatch {
            expected: expected.safepoint_id,
            actual: provisional.header.safepoint_id,
        });
    }
    if provisional.header.owner != expected.owner {
        return Err(StackmapNormalizationError::OwnerMismatch {
            expected: expected.owner,
            actual: provisional.header.owner,
        });
    }
    if provisional.header.flags != 0 {
        return Err(StackmapNormalizationError::NonZeroRecordFlags(
            provisional.header.flags,
        ));
    }
    validate_stack_size(provisional.header.stack_size)?;
    let expected_location_count = usize::try_from(expected.root_pair_count)
        .ok()
        .and_then(|count| count.checked_mul(2))
        .and_then(|count| count.checked_add(3))
        .ok_or(StackmapNormalizationError::LocationCountOverflow)?;
    if provisional.locations.len() != expected_location_count {
        return Err(StackmapNormalizationError::LocationCountMismatch {
            expected: expected_location_count,
            actual: provisional.locations.len(),
        });
    }
    let locations = provisional
        .locations
        .iter()
        .copied()
        .enumerate()
        .map(|(index, location)| normalize_location(index, location, constant_pool))
        .collect::<Result<Vec<_>, _>>()?;
    validate_header_locations(&locations)?;
    validate_root_pairs(&locations[3..], provisional.header.stack_size)?;
    let live_outs = provisional
        .live_outs
        .iter()
        .map(|live_out| CanonicalStackmapLiveOutV1 {
            dwarf_register: u32::from(live_out.dwarf_register),
            size: u32::from(live_out.size),
        })
        .collect();
    let canonical = CanonicalStackmapRecordV1 {
        site: expected.site,
        safepoint_id: provisional.header.safepoint_id,
        owner: provisional.header.owner,
        role: expected.role,
        root_pair_count: expected.root_pair_count,
        instruction_offset: provisional.header.instruction_offset,
        stack_size: provisional.header.stack_size,
        locations,
        live_outs,
    };
    let fingerprint = domain_separated_runtime_hash(STACKMAP_FINGERPRINT_DOMAIN, &canonical)
        .map(|digest| StackmapRecordFingerprintV1(*digest.as_array()))
        .map_err(StackmapNormalizationError::Fingerprint)?;
    Ok(VerifiedNormalizedStackmapRecordV1 {
        canonical,
        fingerprint,
    })
}

fn normalize_location(
    index: usize,
    location: ProvisionalLlvmStackmapLocationV3,
    constants: &[u64],
) -> Result<CanonicalStackmapLocationV1, StackmapNormalizationError> {
    let size = u32::from(location.size);
    let dwarf_register = u32::from(location.dwarf_register);
    let signed_bits = location.offset as i64 as u64;
    match location.kind {
        LOCATION_REGISTER => {
            if location.offset != 0 {
                return Err(StackmapNormalizationError::NonZeroRegisterOffset {
                    index,
                    offset: location.offset,
                });
            }
            Ok(CanonicalStackmapLocationV1::Register {
                size,
                dwarf_register,
            })
        }
        LOCATION_DIRECT => Ok(CanonicalStackmapLocationV1::Direct {
            size,
            dwarf_register,
            signed_offset_bits: signed_bits,
        }),
        LOCATION_INDIRECT => Ok(CanonicalStackmapLocationV1::Indirect {
            size,
            dwarf_register,
            signed_offset_bits: signed_bits,
        }),
        LOCATION_CONSTANT | LOCATION_CONSTANT_INDEX => {
            if location.dwarf_register != 0 {
                return Err(StackmapNormalizationError::NonZeroConstantRegister {
                    index,
                    dwarf_register: location.dwarf_register,
                });
            }
            let value_bits = if location.kind == LOCATION_CONSTANT {
                signed_bits
            } else {
                let constant_index = usize::try_from(location.offset).map_err(|_| {
                    StackmapNormalizationError::InvalidConstantPoolIndex {
                        index,
                        constant_index: location.offset,
                    }
                })?;
                *constants.get(constant_index).ok_or(
                    StackmapNormalizationError::InvalidConstantPoolIndex {
                        index,
                        constant_index: location.offset,
                    },
                )?
            };
            Ok(CanonicalStackmapLocationV1::Constant { size, value_bits })
        }
        kind => Err(StackmapNormalizationError::UnknownLocationKind { index, kind }),
    }
}

fn validate_stack_size(stack_size: u64) -> Result<(), StackmapNormalizationError> {
    if stack_size < 16 || stack_size == u64::MAX || stack_size % 16 != 0 {
        Err(StackmapNormalizationError::InvalidStackSize(stack_size))
    } else {
        Ok(())
    }
}

fn validate_header_locations(
    locations: &[CanonicalStackmapLocationV1],
) -> Result<(), StackmapNormalizationError> {
    let values = locations[..3]
        .iter()
        .enumerate()
        .map(|(index, location)| match location {
            CanonicalStackmapLocationV1::Constant {
                size: 8,
                value_bits,
            } => Ok(*value_bits),
            _ => Err(StackmapNormalizationError::InvalidHeaderLocation { index }),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values[1] != 0 {
        return Err(StackmapNormalizationError::NonZeroStatepointFlags(
            values[1],
        ));
    }
    if values[2] != 0 {
        return Err(StackmapNormalizationError::NonZeroDeoptCount(values[2]));
    }
    Ok(())
}

fn validate_root_pairs(
    locations: &[CanonicalStackmapLocationV1],
    stack_size: u64,
) -> Result<(), StackmapNormalizationError> {
    for (root_index, pair) in locations.chunks_exact(2).enumerate() {
        if pair[0] != pair[1] {
            return Err(StackmapNormalizationError::DistinctRootPair(root_index));
        }
        let CanonicalStackmapLocationV1::Indirect {
            size: 8,
            dwarf_register,
            signed_offset_bits,
        } = pair[0]
        else {
            return Err(StackmapNormalizationError::InvalidRootLocation(root_index));
        };
        let register = u16::try_from(dwarf_register)
            .map_err(|_| StackmapNormalizationError::InvalidRootLocation(root_index))?;
        if !matches!(register, AARCH64_DWARF_SP | AARCH64_DWARF_FP) {
            return Err(StackmapNormalizationError::InvalidRootLocation(root_index));
        }
        let signed_offset = signed_offset_bits as i64;
        let base = if register == AARCH64_DWARF_SP {
            0_i128
        } else {
            i128::from(stack_size) - 16
        };
        let frame_offset = base + i128::from(signed_offset);
        if frame_offset < 0 || frame_offset + 8 > i128::from(stack_size) {
            return Err(StackmapNormalizationError::RootOutsideFrame {
                index: root_index,
                stack_size,
                dwarf_register: register,
                signed_offset,
            });
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StackmapNormalizationError {
    SafepointIdMismatch {
        expected: u64,
        actual: u64,
    },
    OwnerMismatch {
        expected: PersistentCallableBodyId,
        actual: PersistentCallableBodyId,
    },
    NonZeroRecordFlags(u16),
    InvalidStackSize(u64),
    LocationCountOverflow,
    LocationCountMismatch {
        expected: usize,
        actual: usize,
    },
    UnknownLocationKind {
        index: usize,
        kind: u8,
    },
    NonZeroRegisterOffset {
        index: usize,
        offset: i32,
    },
    NonZeroConstantRegister {
        index: usize,
        dwarf_register: u16,
    },
    InvalidConstantPoolIndex {
        index: usize,
        constant_index: i32,
    },
    InvalidHeaderLocation {
        index: usize,
    },
    NonZeroStatepointFlags(u64),
    NonZeroDeoptCount(u64),
    DistinctRootPair(usize),
    InvalidRootLocation(usize),
    RootOutsideFrame {
        index: usize,
        stack_size: u64,
        dwarf_register: u16,
        signed_offset: i64,
    },
    Fingerprint(HashError),
}

impl fmt::Display for StackmapNormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid LLVM v3 stackmap record: {self:?}")
    }
}

impl std::error::Error for StackmapNormalizationError {}

#[cfg(test)]
mod tests;
