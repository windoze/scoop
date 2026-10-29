use super::*;
use crate::link_object::{
    BuiltinObjectSectionRoleV1 as Role, ValidatedBuiltinObjectSectionInventoryV1,
};

pub(crate) fn sections(
    bytes: &[u8],
    target: TargetProfileId,
) -> Result<ValidatedBuiltinObjectSectionInventoryV1, CoffObjectError> {
    let envelope = validate_windows_coff_object_envelope_v1(bytes, target)?;
    let roles = envelope
        .sections()
        .iter()
        .map(|section| {
            let ObjectSectionFlagsV1::Coff { characteristics } = section.flags() else {
                unreachable!("COFF reader produces COFF characteristics")
            };
            classify(section.section_name(), characteristics)
        })
        .collect();
    Ok(ValidatedBuiltinObjectSectionInventoryV1 { envelope, roles })
}

fn classify(name: &[u8], flags: u32) -> Role {
    use object::pe::*;
    if flags & (IMAGE_SCN_LNK_INFO | IMAGE_SCN_LNK_REMOVE) != 0
        || flags & (IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE | IMAGE_SCN_MEM_EXECUTE) == 0
        || name.starts_with(b".debug")
    {
        return Role::ObjectMetadata;
    }
    if name == b".llvm_stackmaps" {
        return Role::LlvmStackmaps;
    }
    if contribution(name, b".pdata") {
        return Role::WindowsUnwindTable;
    }
    if contribution(name, b".xdata") {
        return Role::WindowsUnwindInfo;
    }
    let zero_fill = flags & IMAGE_SCN_CNT_UNINITIALIZED_DATA != 0;
    if contribution(name, b".tls") {
        if zero_fill {
            Role::ThreadLocalZeroFill
        } else {
            Role::ThreadLocalData
        }
    } else if zero_fill {
        Role::ZeroFill
    } else if flags & IMAGE_SCN_MEM_EXECUTE != 0 {
        Role::Text
    } else if flags & IMAGE_SCN_MEM_WRITE != 0 {
        Role::WritableData
    } else {
        Role::ReadOnlyData
    }
}

fn contribution(name: &[u8], base: &[u8]) -> bool {
    name == base
        || name
            .strip_prefix(base)
            .is_some_and(|suffix| suffix.starts_with(b"$"))
}
