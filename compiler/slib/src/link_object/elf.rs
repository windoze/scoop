//! Shared ELF64 relocatable-file bounds and references, independent of LLVM.
use object::read::elf::ElfFile64;
use object::{Architecture, Object, ObjectKind, SectionIndex, SymbolIndex};
use scoop_identity::TargetProfileId;

mod relocations;
mod sections;
mod x86_64;

#[derive(Debug)]
pub struct ValidatedElfObject<'data> {
    file: ElfFile64<'data>,
    relocations: Vec<ElfRelocation>,
}

#[derive(Clone, Copy, Debug)]
pub struct ElfRelocation {
    pub section: SectionIndex,
    pub offset: u64,
    pub symbol: SymbolIndex,
    pub addend: i64,
    pub kind: u32,
    pub width: u8,
}

impl<'data> ValidatedElfObject<'data> {
    pub fn read(bytes: &'data [u8], target: TargetProfileId) -> Result<Self, ElfObjectError> {
        let architecture = match target {
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => {
                Architecture::X86_64
            }
            TargetProfileId::DarwinAarch64 => return Err(error("target does not use ELF")),
        };
        let file = ElfFile64::parse(bytes).map_err(error)?;
        if !file.is_little_endian()
            || file.kind() != ObjectKind::Relocatable
            || file.architecture() != architecture
        {
            return Err(error(
                "expected a little-endian ELF64 relocatable object for the selected target",
            ));
        }
        sections::check(&file)?;
        let relocations = relocations::read(&file)?;
        Ok(Self { file, relocations })
    }

    pub fn file(&self) -> &ElfFile64<'data> {
        &self.file
    }

    pub fn relocations(&self) -> &[ElfRelocation] {
        &self.relocations
    }
}

#[derive(Debug)]
pub struct ElfObjectError(String);

impl std::fmt::Display for ElfObjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "ELF object: {}", self.0)
    }
}
impl std::error::Error for ElfObjectError {}

fn error(message: impl std::fmt::Display) -> ElfObjectError {
    ElfObjectError(message.to_string())
}
