use super::*;

impl NativeObjectIndex {
    /// ELF indexing already checked its relocations; only Mach-O final-image
    /// inspection needs the reconstructed instruction references returned here.
    pub fn check_selected(&self, bytes: &[u8]) -> Result<Option<NativeReferences>, LinkError> {
        let (common, implicit_inputs) = match &self.selection {
            NativeSelectionChecks::Darwin {
                common,
                implicit_inputs,
            } => (common, implicit_inputs),
            NativeSelectionChecks::Elf { rejection } => {
                return match rejection {
                    Some(message) => Err(error(message)),
                    None => Ok(None),
                };
            }
        };
        if let Some(name) = common.first() {
            return Err(error(format!(
                "native common/tentative definition {name} requires actual storage"
            )));
        }
        if !implicit_inputs.is_empty() {
            return Err(error(format!(
                "native object requests an implicit linker input or unsupported load command: {:?}",
                implicit_inputs
            )));
        }
        for name in self.info.definitions.keys().chain(&self.info.requirements) {
            if name.starts_with("___cxa_")
                || name.starts_with("___gxx_personality")
                || name.starts_with("___gcc_personality")
                || name.starts_with("__ZSt9terminate")
            {
                return Err(error(format!(
                    "native object has forbidden C++ ABI dependency {name}"
                )));
            }
        }
        let file: MachOFile64<'_> = MachOFile64::parse(bytes).map_err(error)?;
        for section in file.sections() {
            let name = section.name().map_err(error)?;
            let kind = section.macho_section().flags.get(file.endian()) & macho::SECTION_TYPE;
            if matches!(
                name,
                "__mod_init_func" | "__mod_term_func" | "__thread_init" | "__init_offsets"
            ) || matches!(
                kind,
                macho::S_MOD_INIT_FUNC_POINTERS
                    | macho::S_MOD_TERM_FUNC_POINTERS
                    | macho::S_THREAD_LOCAL_INIT_FUNCTION_POINTERS
                    | macho::S_INIT_FUNC_OFFSETS
            ) || section.segment_name().map_err(error)? == Some("__LLVM")
                || name.starts_with("__objc_")
                || name.starts_with("__swift")
            {
                return Err(error(format!(
                    "native object contains forbidden initialization or LTO section {name}"
                )));
            }
        }
        NativeReferences::read(&file).map(Some)
    }
}
