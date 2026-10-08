use super::*;
use crate::native_input::{NativeContent, NativeFileKind};
use crate::program::DefinitionOwner;

impl ElfNamespace {
    pub(super) fn read_graph(
        &mut self,
        native: &mut NativeInputs,
        roots: &[PathBuf],
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<(), LinkError> {
        self.roots = native
            .ordered_files()
            .iter()
            .filter(|file| {
                matches!(file.content, NativeContent::ElfDynamic(_))
                    && !self.system_inputs.contains_key(&file.id)
            })
            .map(|file| file.id)
            .collect();
        let mut names = BTreeMap::new();
        for id in self.roots.clone() {
            self.insert(native, id, &mut names)?;
        }
        let mut pending: BTreeSet<_> = self.roots.iter().copied().collect();
        let mut visited = BTreeSet::new();
        while let Some(id) = pending.pop_first() {
            if !visited.insert(id) {
                continue;
            }
            let interface = self.providers[&id].interface.clone();
            let mut edges = Vec::new();
            for name in &interface.needed {
                if self.needed.contains(name) {
                    continue;
                }
                if name.starts_with("libgcc_s.so") {
                    return Err(error(
                        "native DSO requires a second EH provider alongside LLVM libunwind",
                    ));
                }
                let child = if let Some(child) = names.get(name) {
                    *child
                } else {
                    let file = self.dependency(&self.providers[&id], name, roots, profile)?;
                    let child = file.id;
                    native.files.entry(child).or_insert(file);
                    self.insert(native, child, &mut names)?;
                    if self.providers[&child].name != *name {
                        return Err(error(format!(
                            "ELF dependency {name} has different SONAME {}",
                            self.providers[&child].name
                        )));
                    }
                    child
                };
                edges.push(child);
                pending.insert(child);
            }
            self.edges.insert(id, edges);
        }
        Ok(())
    }

    fn insert(
        &mut self,
        native: &NativeInputs,
        id: NativeInputId,
        names: &mut BTreeMap<String, NativeInputId>,
    ) -> Result<(), LinkError> {
        let file = &native.files[&id];
        let NativeContent::ElfDynamic(interface) = &file.content else {
            return Err(error("ELF dependency resolved to a static input"));
        };
        let name = match &interface.soname {
            Some(name) => name.clone(),
            None => file
                .locator
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| error("ELF library without SONAME requires a UTF-8 filename"))?
                .to_owned(),
        };
        if self.needed.contains(&name) {
            return Err(error(format!(
                "native ELF library {name} conflicts with the selected system library"
            )));
        }
        if let Some(previous) = names.insert(name.clone(), id)
            && previous != id
        {
            return Err(error(format!(
                "conflicting ELF providers for SONAME {name}"
            )));
        }
        self.providers.entry(id).or_insert_with(|| ElfProvider {
            name,
            locator: file.locator.clone(),
            interface: interface.clone(),
        });
        Ok(())
    }

    fn dependency(
        &self,
        provider: &ElfProvider,
        name: &str,
        roots: &[PathBuf],
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<crate::native_input::NativeFile, LinkError> {
        let parent = provider
            .locator
            .parent()
            .ok_or_else(|| error("ELF provider has no parent directory"))?;
        let expand = |value: &str| {
            value
                .replace("${ORIGIN}", &parent.to_string_lossy())
                .replace("$ORIGIN", &parent.to_string_lossy())
        };
        let name_path = PathBuf::from(expand(name));
        let paths: BTreeSet<_> = if name_path.is_absolute() || name.contains('/') {
            BTreeSet::from([name_path])
        } else {
            std::iter::once(parent.to_owned())
                .chain(roots.iter().cloned())
                .chain(
                    provider
                        .interface
                        .runpaths
                        .iter()
                        .filter(|value| !value.is_empty())
                        .map(|value| PathBuf::from(expand(value))),
                )
                .chain(
                    self.paths
                        .iter()
                        .filter_map(|path| path.parent().map(PathBuf::from)),
                )
                .map(|root| root.join(&name_path))
                .collect()
        };
        let mut candidates = BTreeMap::new();
        for path in paths.iter().filter(|path| path.is_file()) {
            let file = crate::native_input::locate::read(
                path,
                NativeFileKind::SharedObject,
                profile,
                false,
            )
            .map_err(|err| {
                error(format!(
                    "ELF dependency {} for {}: {err}",
                    path.display(),
                    provider.name
                ))
            })?;
            candidates.entry(file.id).or_insert(file);
        }
        if candidates.len() != 1 {
            return Err(error(format!(
                "ELF dependency {name} for {} has {} candidates in {paths:?}",
                provider.name,
                candidates.len()
            )));
        }
        candidates
            .into_values()
            .next()
            .ok_or_else(|| error("ELF dependency disappeared"))
    }

    pub fn project(
        &mut self,
        definitions: &BTreeMap<String, DefinitionOwner>,
    ) -> Result<(), LinkError> {
        let mut pending: BTreeSet<_> = self
            .bindings
            .values()
            .filter_map(|binding| match binding {
                ElfBinding::Shared { input, .. } => Some(*input),
                ElfBinding::System(_) => None,
            })
            .collect();
        while let Some(id) = pending.pop_first() {
            if self.selected.insert(id) {
                pending.extend(&self.edges[&id]);
            }
        }
        let order = self.selected_order();
        for (symbol, binding) in &self.bindings {
            if let ElfBinding::Shared { input, .. } = binding
                && let Some(actual) = order.iter().find(|id| {
                    self.providers[id]
                        .interface
                        .symbols
                        .find(symbol, None)
                        .is_some()
                })
                && actual != input
            {
                return Err(error(format!(
                    "ELF native symbol {symbol} conflicts: {} is searched before required {}",
                    self.providers[actual].name, self.providers[input].name
                )));
            }
        }
        for id in &self.selected {
            let provider = &self.providers[id];
            for import in &provider.interface.symbols.imports {
                if import.weak
                    || (import.version.is_none() && definitions.contains_key(&import.name))
                    || self.system_import(import)
                {
                    continue;
                }
                if self.selected.iter().any(|id| {
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
                }) {
                    continue;
                }
                return Err(error(format!(
                    "unresolved ELF import {} version {:?} from {}",
                    import.name, import.version, provider.name
                )));
            }
        }
        for id in &self.selected {
            let provider = &self.providers[id];
            self.needed.insert(provider.name.clone());
            self.rpaths.insert(
                provider
                    .locator
                    .parent()
                    .ok_or_else(|| error("ELF library has no parent directory"))?
                    .to_owned(),
            );
        }
        Ok(())
    }

    pub fn import_requirements(&mut self, binding: &ElfBinding) -> Vec<String> {
        let ElfBinding::Shared { input, .. } = binding else {
            return Vec::new();
        };
        let mut closure = BTreeSet::new();
        let mut pending = vec![*input];
        while let Some(id) = pending.pop() {
            if closure.insert(id) {
                pending.extend(&self.edges[&id]);
            }
        }
        let mut requirements = BTreeSet::new();
        for id in &closure {
            if !self.expanded.insert(*id) {
                continue;
            }
            for import in &self.providers[id].interface.symbols.imports {
                if import.weak || import.version.is_some() || self.system_import(import) {
                    continue;
                }
                if !closure.iter().any(|id| {
                    self.providers[id]
                        .interface
                        .symbols
                        .find(&import.name, None)
                        .is_some()
                }) {
                    requirements.insert(import.name.clone());
                }
            }
        }
        requirements.into_iter().collect()
    }

    pub fn selected_order(&self) -> Vec<NativeInputId> {
        let mut pending = std::collections::VecDeque::from(self.roots.clone());
        let mut seen = BTreeSet::new();
        let mut result = Vec::new();
        while let Some(id) = pending.pop_front() {
            if !self.selected.contains(&id) || !seen.insert(id) {
                continue;
            }
            result.push(id);
            pending.extend(&self.edges[&id]);
        }
        result
    }
}
