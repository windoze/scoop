use super::*;
use crate::link_object::{
    BuiltinObjectSectionRoleV1 as Role, ObjectSectionFlagsV1,
    ValidatedBuiltinObjectSectionInventoryV1,
};

pub(crate) fn sections(
    bytes: &[u8],
    target: TargetProfileId,
) -> Result<ValidatedBuiltinObjectSectionInventoryV1, ElfObjectError> {
    let envelope = validate_linux_elf_object_envelope_v1(bytes, target)?;
    let roles = envelope
        .sections()
        .iter()
        .map(|section| {
            let ObjectSectionFlagsV1::Elf {
                section_type,
                flags,
            } = section.flags()
            else {
                unreachable!("ELF reader produces ELF section flags")
            };
            classify(section.section_name(), section_type, flags)
        })
        .collect();
    Ok(ValidatedBuiltinObjectSectionInventoryV1 { envelope, roles })
}

fn classify(name: &[u8], kind: u32, flags: u64) -> Role {
    use object::elf::*;
    if flags & u64::from(SHF_ALLOC) == 0 {
        return Role::ObjectMetadata;
    }
    match name {
        b".llvm_stackmaps" => return Role::LlvmStackmaps,
        b".eh_frame" => return Role::EhFrame,
        _ if name.starts_with(b".gcc_except_table") => return Role::GccExceptionTable,
        _ => {}
    }
    if flags & u64::from(SHF_TLS) != 0 {
        if kind == SHT_NOBITS {
            Role::ThreadLocalZeroFill
        } else {
            Role::ThreadLocalData
        }
    } else if kind == SHT_NOBITS {
        Role::ZeroFill
    } else if flags & u64::from(SHF_EXECINSTR) != 0 {
        Role::Text
    } else if flags & u64::from(SHF_STRINGS) != 0 || name.starts_with(b".rodata.str") {
        Role::CString
    } else if flags & u64::from(SHF_WRITE) == 0 || name.starts_with(b".data.rel.ro") {
        Role::ReadOnlyData
    } else {
        Role::WritableData
    }
}
