use super::*;

impl ElfNamespace {
    pub fn check_final(&self, file: &ElfFile64<'_>) -> Result<(), LinkError> {
        let symbols = crate::native_input::elf_dynamic::read_symbols(file)?;
        for import in &symbols.imports {
            if import.weak {
                continue;
            }
            let found = self.system_import(import)
                || self.selected.iter().any(|id| {
                    let provider = &self.providers[id];
                    import
                        .library
                        .as_ref()
                        .is_none_or(|name| name == &provider.name)
                        && provider
                            .interface
                            .symbols
                            .find(&import.name, import.version.as_deref())
                            .is_some()
                });
            if !found {
                return Err(error(format!(
                    "unresolved final ELF import {} version {:?}",
                    import.name, import.version
                )));
            }
            if let Some(ElfBinding::Shared { input, export }) = self.bindings.get(&import.name)
                && (export.version != import.version
                    || import
                        .library
                        .as_ref()
                        .is_some_and(|name| name != &self.providers[input].name))
            {
                return Err(error(format!(
                    "final ELF import {} has a different symbol version or library",
                    import.name
                )));
            }
        }
        let mut needed = BTreeSet::new();
        let mut rpaths = BTreeSet::new();
        for (tag, value) in dynamic_strings(file)? {
            match tag {
                elf::DT_NEEDED => {
                    if !self.needed.contains(&value) {
                        return Err(error(format!("unexpected ELF dependency {value}")));
                    }
                    needed.insert(value);
                }
                elf::DT_RPATH | elf::DT_RUNPATH => {
                    rpaths.extend(value.split(':').map(PathBuf::from))
                }
                _ => {}
            }
        }
        for id in &self.selected {
            if !needed.contains(&self.providers[id].name) {
                return Err(error(format!(
                    "ELF output omitted shared library {}",
                    self.providers[id].name
                )));
            }
        }
        if rpaths != self.rpaths {
            return Err(error(
                "ELF output has different native library search paths",
            ));
        }
        Ok(())
    }
}
