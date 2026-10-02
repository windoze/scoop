use super::*;

impl NativeObjectIndex {
    pub fn check_selected(&self, bytes: &[u8]) -> Result<(), LinkError> {
        if let Some(name) = self.common.first() {
            return Err(error(format!(
                "native common/tentative definition {name} requires actual storage"
            )));
        }
        if !self.implicit_inputs.is_empty() {
            return Err(error("native object requests an implicit linker input"));
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
            if matches!(
                name,
                "__mod_init_func" | "__mod_term_func" | "__thread_init" | "__init_offsets"
            ) || section.segment_name().map_err(error)? == Some("__LLVM")
                || name.starts_with("__objc_")
                || name.starts_with("__swift")
            {
                return Err(error(format!(
                    "native object contains forbidden initialization or LTO section {name}"
                )));
            }
            section.data().map_err(error)?;
            for (offset, relocation) in section.relocations() {
                if offset
                    .checked_add(u64::from(relocation.size().div_ceil(8)))
                    .is_none_or(|end| end > section.size())
                {
                    return Err(error(format!("native relocation outside {name}")));
                }
                match relocation.target() {
                    RelocationTarget::Symbol(index) => {
                        file.symbol_by_index(index).map_err(error)?;
                    }
                    RelocationTarget::Section(index) => {
                        file.section_by_index(index).map_err(error)?;
                    }
                    RelocationTarget::Absolute => {}
                    _ => return Err(error("unknown native relocation target")),
                }
            }
        }
        Ok(())
    }
}
