use super::{ElfObjectError, error};
use object::elf::*;

pub(super) fn relocation_width(kind: u32) -> Result<u8, ElfObjectError> {
    Ok(match kind {
        R_X86_64_NONE | R_X86_64_TLSDESC_CALL => 0,
        R_X86_64_8 | R_X86_64_PC8 => 1,
        R_X86_64_16 | R_X86_64_PC16 => 2,
        R_X86_64_PC32
        | R_X86_64_GOT32
        | R_X86_64_PLT32
        | R_X86_64_GOTPCREL
        | R_X86_64_32
        | R_X86_64_32S
        | R_X86_64_TLSGD
        | R_X86_64_TLSLD
        | R_X86_64_DTPOFF32
        | R_X86_64_GOTTPOFF
        | R_X86_64_TPOFF32
        | R_X86_64_GOTPC32
        | R_X86_64_SIZE32
        | R_X86_64_GOTPC32_TLSDESC
        | R_X86_64_GOTPCRELX
        | R_X86_64_REX_GOTPCRELX => 4,
        // LLVM 22/GCC also emit the instruction-prefix variants absent from
        // object 0.37's named constants; their relocation field is still disp32.
        43..=45 | 50 => 4,
        R_X86_64_64 | R_X86_64_DTPOFF64 | R_X86_64_TPOFF64 | R_X86_64_PC64 | R_X86_64_GOTOFF64
        | R_X86_64_GOT64 | R_X86_64_GOTPCREL64 | R_X86_64_GOTPC64 | R_X86_64_GOTPLT64
        | R_X86_64_PLTOFF64 | R_X86_64_SIZE64 => 8,
        _ => return Err(error(format!("unsupported amd64 ET_REL relocation {kind}"))),
    })
}
