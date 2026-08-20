use super::*;
use crate::link_object::{
    BuiltinLinkObjectSectionProfileV1 as Profile, BuiltinObjectSectionRoleV1 as Role,
    ObjectSectionFlagsV1, ValidatedBuiltinObjectSectionInventoryV1,
};

pub(crate) fn sections(
    bytes: &[u8],
    target: TargetProfileId,
    profile: Profile,
) -> Result<ValidatedBuiltinObjectSectionInventoryV1, ElfObjectError> {
    let envelope = validate_linux_elf_object_envelope_v1(bytes, target)?;
    let mut roles = Vec::new();
    for section in envelope.sections() {
        let ObjectSectionFlagsV1::Elf {
            section_type,
            flags,
        } = section.flags()
        else {
            return Err(error("non-ELF section in ELF inventory"));
        };
        let name = section.section_name();
        let role = classify(name, section_type, flags)?;
        if profile == Profile::GeneratedCBridge
            && section.byte_size() != 0
            && !matches!(
                role,
                Role::Text | Role::ReadOnlyData | Role::CString | Role::ObjectMetadata
            )
        {
            return Err(error(format!(
                "unexpected generated-C section {}",
                String::from_utf8_lossy(name)
            )));
        }
        roles.push(role);
    }
    if profile == Profile::GeneratedCBridge && !roles.contains(&Role::Text) {
        return Err(error("generated-C object has no code section"));
    }
    Ok(ValidatedBuiltinObjectSectionInventoryV1 {
        envelope,
        profile,
        roles,
    })
}

fn classify(name: &[u8], kind: u32, flags: u64) -> Result<Role, ElfObjectError> {
    use object::elf::*;
    if name == b".note.gnu.property" && kind == SHT_NOTE && flags == u64::from(SHF_ALLOC) {
        return Ok(Role::ObjectMetadata);
    }
    if flags & u64::from(SHF_ALLOC) == 0 {
        if matches!(kind, SHT_SYMTAB | SHT_STRTAB | SHT_RELA | SHT_GROUP)
            || (kind == SHT_PROGBITS
                && (matches!(name, b".note.GNU-stack" | b".comment")
                    || name.starts_with(b".debug")))
        {
            return Ok(Role::ObjectMetadata);
        }
        return Err(error(format!(
            "unexpected non-allocated ELF section {}",
            String::from_utf8_lossy(name)
        )));
    }
    let (role, required) = if name == b".text" || name.starts_with(b".text.") {
        (Role::Text, SHF_ALLOC | SHF_EXECINSTR)
    } else if name.starts_with(b".rodata.str") {
        let strings = SHF_MERGE | SHF_STRINGS;
        let required = if flags & u64::from(strings) == 0 {
            SHF_ALLOC
        } else {
            SHF_ALLOC | strings
        };
        (Role::CString, required)
    } else if matches!(name, b".rodata.cst4" | b".rodata.cst8" | b".rodata.cst16") {
        (Role::ReadOnlyData, SHF_ALLOC | SHF_MERGE)
    } else if name == b".rodata" || name.starts_with(b".rodata.") {
        (Role::ReadOnlyData, SHF_ALLOC)
    } else if name.starts_with(b".data.rel.ro.") {
        (Role::ReadOnlyData, SHF_ALLOC | SHF_WRITE)
    } else if name == b".data" || name.starts_with(b".data.") {
        (Role::WritableData, SHF_ALLOC | SHF_WRITE)
    } else if name == b".bss" || name.starts_with(b".bss.") {
        (Role::ZeroFill, SHF_ALLOC | SHF_WRITE)
    } else if name == b".tdata" || name.starts_with(b".tdata.") {
        (Role::ThreadLocalData, SHF_ALLOC | SHF_WRITE | SHF_TLS)
    } else if name == b".tbss" || name.starts_with(b".tbss.") {
        (Role::ThreadLocalZeroFill, SHF_ALLOC | SHF_WRITE | SHF_TLS)
    } else if name == b".llvm_stackmaps" {
        (Role::LlvmStackmaps, SHF_ALLOC | SHF_WRITE)
    } else if name == b".eh_frame" {
        (Role::EhFrame, SHF_ALLOC)
    } else if name == b".gcc_except_table" || name.starts_with(b".gcc_except_table.") {
        (Role::GccExceptionTable, SHF_ALLOC)
    } else {
        return Err(error(format!(
            "unexpected allocated ELF section {}",
            String::from_utf8_lossy(name)
        )));
    };
    let expected_kind = if matches!(role, Role::ZeroFill | Role::ThreadLocalZeroFill) {
        SHT_NOBITS
    } else {
        SHT_PROGBITS
    };
    let valid_kind = kind == expected_kind || (role == Role::EhFrame && kind == SHT_X86_64_UNWIND);
    if !valid_kind || flags & !u64::from(SHF_GROUP) != u64::from(required) {
        return Err(error(format!(
            "invalid flags/type ({flags:#x}/{kind}) for ELF section {}",
            String::from_utf8_lossy(name)
        )));
    }
    Ok(role)
}
