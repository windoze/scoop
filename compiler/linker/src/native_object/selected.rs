use super::*;

impl NativeObjectIndex {
    /// Check the selected native object, without retaining a final-image replay.
    pub fn check_selected(&self, bytes: &[u8]) -> Result<(), LinkError> {
        let (common, implicit_inputs, cxx) = match &self.selection {
            NativeSelectionChecks::Darwin {
                common,
                implicit_inputs,
                cxx,
            } => (common, implicit_inputs, *cxx),
            NativeSelectionChecks::Elf { rejection } => {
                return match rejection {
                    Some(message) => Err(error(message)),
                    None => Ok(()),
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
            if !cxx
                && ((name.starts_with("___cxa_")
                    && !matches!(name.as_str(), "___cxa_atexit" | "___cxa_finalize"))
                    || name.starts_with("___gxx_personality")
                    || name.starts_with("___gcc_personality")
                    || name.starts_with("__ZSt9terminate"))
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
            if (!cxx
                && (name == "__thread_init"
                    || kind == macho::S_THREAD_LOCAL_INIT_FUNCTION_POINTERS))
                || section.segment_name().map_err(error)? == Some("__LLVM")
                || name.starts_with("__objc_")
                || name.starts_with("__swift")
            {
                return Err(error(format!(
                    "native object contains forbidden initialization or LTO section {name}"
                )));
            }
        }
        references::check(&file)
    }
}
