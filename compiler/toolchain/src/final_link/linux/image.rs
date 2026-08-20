use super::*;
use object::read::elf::{Dyn, ElfFile64, ProgramHeader};
use object::{Architecture, Object, ObjectKind, ObjectSymbol, elf};

pub(super) fn check(profile: &LinuxFinalLinkProfile, bytes: &[u8]) -> Result<(), ToolchainError> {
    let file: ElfFile64<'_> = ElfFile64::parse(bytes).map_err(error)?;
    let expected = match profile.mode {
        LinkMode::Static => ObjectKind::Executable,
        LinkMode::Dynamic => ObjectKind::Dynamic,
    };
    if file.architecture() != Architecture::X86_64
        || !file.is_little_endian()
        || file.kind() != expected
    {
        return Err(error(
            "linker did not produce the selected ELF64 amd64 executable mode",
        ));
    }
    let endian = file.endian();
    let mut interpreter = None;
    let mut eh_frame_header = false;
    let mut read_only_after_relocation = false;
    for segment in file.elf_program_headers() {
        match segment.p_type(endian) {
            elf::PT_INTERP => {
                if interpreter.is_some() {
                    return Err(error("ELF executable has multiple interpreters"));
                }
                interpreter = segment.interpreter(endian, bytes).map_err(error)?;
            }
            elf::PT_GNU_EH_FRAME => eh_frame_header = true,
            elf::PT_GNU_RELRO => read_only_after_relocation = true,
            elf::PT_GNU_STACK if segment.p_flags(endian) & elf::PF_X != 0 => {
                return Err(error("ELF executable requests an executable stack"));
            }
            _ => {}
        }
        if let Some(entries) = segment.dynamic(endian, bytes).map_err(error)? {
            for entry in entries {
                let tag = entry.d_tag(endian);
                if tag == u64::from(elf::DT_NULL) {
                    break;
                }
                if tag == u64::from(elf::DT_TEXTREL)
                    || (tag == u64::from(elf::DT_FLAGS)
                        && entry.d_val(endian) & u64::from(elf::DF_TEXTREL) != 0)
                {
                    return Err(error("ELF executable contains text relocations"));
                }
                if profile.mode == LinkMode::Static && tag == u64::from(elf::DT_NEEDED) {
                    return Err(error("static ELF executable has dynamic dependencies"));
                }
            }
        }
    }
    if !eh_frame_header {
        return Err(error("ELF executable is missing PT_GNU_EH_FRAME"));
    }
    match (profile.mode, interpreter) {
        (LinkMode::Static, None) => {}
        (LinkMode::Dynamic, Some(interpreter)) => {
            let interpreter = std::str::from_utf8(interpreter).map_err(error)?;
            let expected = match profile.startup.profile().contract().target().id() {
                TargetProfileId::LinuxX86_64Gnu => "ld-linux-x86-64.so.2",
                TargetProfileId::LinuxX86_64Musl => "ld-musl-x86_64.so.1",
                TargetProfileId::DarwinAarch64 => {
                    return Err(error("ELF profile has a Darwin target"));
                }
            };
            if !Path::new(interpreter).is_absolute()
                || Path::new(interpreter).file_name() != Some(std::ffi::OsStr::new(expected))
                || !read_only_after_relocation
            {
                return Err(error(
                    "ELF PIE has the wrong libc interpreter or no GNU_RELRO segment",
                ));
            }
        }
        _ => {
            return Err(error(
                "ELF interpreter does not match the selected link mode",
            ));
        }
    }
    for symbol in file
        .dynamic_symbols()
        .filter(|symbol| symbol.is_undefined())
    {
        let name = symbol.name().map_err(error)?;
        if !profile.cxx()
            && (name.starts_with("_Unwind_")
                || (name.starts_with("__cxa_")
                    && !matches!(name, "__cxa_finalize" | "__cxa_atexit"))
                || name.starts_with("__gxx_personality"))
        {
            return Err(error(format!(
                "ELF executable imports an unexpected EH provider symbol {name}"
            )));
        }
    }
    Ok(())
}
