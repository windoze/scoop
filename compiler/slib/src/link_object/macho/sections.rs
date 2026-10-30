//! Classify sections needed by object consumers without producer allowlists.

use super::ValidatedMachOObjectEnvelopeV1;
use crate::link_object::object_sections::*;
use object::macho;

pub fn read_builtin_object_sections_v1(
    envelope: ValidatedMachOObjectEnvelopeV1,
) -> ValidatedBuiltinObjectSectionInventoryV1 {
    let roles = envelope
        .sections()
        .iter()
        .map(|section| {
            classify_section(
                section.segment_name(),
                section.section_name(),
                section.flags(),
            )
        })
        .collect();
    ValidatedBuiltinObjectSectionInventoryV1 {
        envelope: envelope.into(),
        roles,
    }
}

fn classify_section(segment: &[u8], section: &[u8], flags: u32) -> BuiltinObjectSectionRoleV1 {
    use BuiltinObjectSectionRoleV1 as Role;
    match (segment, section) {
        (b"__LLVM_STACKMAPS", b"__llvm_stackmaps") => return Role::LlvmStackmaps,
        (b"__LD", b"__compact_unwind") => return Role::CompactUnwind,
        (b"__TEXT", b"__eh_frame") => return Role::EhFrame,
        (b"__TEXT", b"__gcc_except_tab") => return Role::GccExceptionTable,
        _ => {}
    }
    match flags & macho::SECTION_TYPE {
        macho::S_ZEROFILL | macho::S_GB_ZEROFILL => Role::ZeroFill,
        macho::S_THREAD_LOCAL_REGULAR => Role::ThreadLocalData,
        macho::S_THREAD_LOCAL_ZEROFILL => Role::ThreadLocalZeroFill,
        macho::S_THREAD_LOCAL_VARIABLES => Role::ThreadLocalVariables,
        macho::S_CSTRING_LITERALS => Role::CString,
        macho::S_4BYTE_LITERALS | macho::S_8BYTE_LITERALS | macho::S_16BYTE_LITERALS => {
            Role::ReadOnlyData
        }
        _ if flags & (macho::S_ATTR_PURE_INSTRUCTIONS | macho::S_ATTR_SOME_INSTRUCTIONS) != 0 => {
            Role::Text
        }
        _ if flags & macho::S_ATTR_DEBUG != 0 => Role::ObjectMetadata,
        _ if segment == b"__TEXT" || segment == b"__DATA_CONST" || section == b"__const" => {
            Role::ReadOnlyData
        }
        _ => Role::WritableData,
    }
}

#[cfg(test)]
mod tests;
