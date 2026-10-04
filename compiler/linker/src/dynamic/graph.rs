use super::*;
use crate::native_input::{NativeContent, NativeFileKind};

impl DynamicInputs {
    pub fn read(
        native: &mut NativeInputs,
        roots: &[PathBuf],
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<Self, LinkError> {
        let system = profile.system_provider();
        let file = system
            .files()
            .iter()
            .find(|file| file.relative_path() == system.root_stub())
            .ok_or_else(|| error("system provider has no root stub"))?;
        let input = NativeInputId::from_bytes(file.bytes(), NativeFileKind::TextStub, profile)?;
        let id = DynamicProvider::id(input, system.install_name(), profile)?;
        let provider = Arc::new(DynamicProvider {
            id,
            input,
            install_name: system.install_name().into(),
            current_version: system.current_version(),
            compatibility_version: system.compatibility_version(),
            exports: system
                .exports()
                .iter()
                .map(|(name, interface)| {
                    (
                        name.clone(),
                        DynamicExport::Symbol {
                            interface: *interface,
                            storage: ExportStorage::InterfaceOnly,
                        },
                    )
                })
                .collect(),
            dependencies: Vec::new(),
            rpaths: Vec::new(),
            locator: profile
                .startup_toolchain()
                .sdk_root()
                .map_err(error)?
                .join(system.root_stub()),
        });
        let mut result = Self {
            providers: BTreeMap::from([(id, provider)]),
            roots: BTreeSet::from([id]),
            edges: BTreeMap::new(),
            rpaths: BTreeSet::new(),
            stubs: BTreeMap::new(),
        };
        let mut names = BTreeMap::from([(system.install_name().to_owned(), id)]);
        for file in native.ordered_files() {
            if let NativeContent::Dynamic(records) = &file.content {
                let root = records
                    .first()
                    .ok_or_else(|| error("dynamic library has no root record"))?;
                locate::runpath(root, roots)?;
                result.roots.insert(root.id);
                for record in records {
                    result.insert(record.clone(), &mut names)?;
                }
            }
        }
        let mut pending: BTreeSet<_> = result.providers.keys().copied().collect();
        let mut visited = BTreeSet::new();
        while let Some(id) = pending.pop_first() {
            if !visited.insert(id) {
                continue;
            }
            let provider = result.providers[&id].clone();
            for (index, edge) in provider.dependencies.iter().enumerate() {
                let target = if let Some(target) = names.get(&edge.name) {
                    *target
                } else {
                    let file = locate::dependency(&provider, &edge.name, roots, profile)?;
                    let NativeContent::Dynamic(records) = &file.content else {
                        return Err(error("dynamic dependency resolved to a static input"));
                    };
                    let target = records
                        .iter()
                        .find(|record| record.install_name == edge.name)
                        .or_else(|| edge.name.starts_with("@loader_path/").then(|| &records[0]))
                        .ok_or_else(|| {
                            error(format!("dynamic file does not contain {}", edge.name))
                        })?
                        .id;
                    for record in records {
                        result.insert(record.clone(), &mut names)?;
                        pending.insert(record.id);
                    }
                    native.files.entry(file.id).or_insert(file);
                    target
                };
                let child = &result.providers[&target];
                if child.current_version < edge.compatibility_version {
                    return Err(error(format!(
                        "dynamic dependency {} has incompatible version for {}",
                        child.install_name, provider.install_name
                    )));
                }
                result.edges.insert((id, index), target);
            }
        }
        Ok(result)
    }

    fn insert(
        &mut self,
        record: Arc<DynamicProvider>,
        names: &mut BTreeMap<String, NativeDynamicProviderId>,
    ) -> Result<(), LinkError> {
        if let Some(previous) = names.insert(record.install_name.clone(), record.id)
            && previous != record.id
        {
            return Err(error(format!(
                "conflicting native providers for install name {}: {} and {}",
                record.install_name,
                self.providers[&previous].locator.display(),
                record.locator.display()
            )));
        }
        self.providers.entry(record.id).or_insert(record);
        Ok(())
    }

    pub fn project(
        &mut self,
        bindings: &BTreeMap<String, DynamicBinding>,
        roots: &[PathBuf],
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<(), LinkError> {
        let mut imports: BTreeMap<_, BTreeMap<_, _>> = BTreeMap::new();
        for (symbol, binding) in bindings {
            imports
                .entry(binding.owner)
                .or_default()
                .insert(symbol.clone(), binding.interface);
        }
        // The fixed provider remains an explicit load input even without calls.
        let system = self
            .providers
            .values()
            .find(|provider| provider.install_name == profile.system_provider().install_name())
            .ok_or_else(|| error("missing fixed system provider"))?;
        imports.entry(system.id).or_default();
        for id in imports.keys() {
            if let Some(path) = locate::runpath(&self.providers[id], roots)? {
                self.rpaths.insert(path);
            }
        }
        let mut pending: BTreeSet<_> = imports.keys().copied().collect();
        let mut visited = BTreeSet::new();
        while let Some(id) = pending.pop_first() {
            if !visited.insert(id) {
                continue;
            }
            let provider = &self.providers[&id];
            for (index, edge) in provider.dependencies.iter().enumerate() {
                let child = self.edges[&(id, index)];
                if edge.name.starts_with("@rpath/") {
                    if let Some(path) = locate::runpath(&self.providers[&child], roots)? {
                        self.rpaths.insert(path);
                    }
                }
                pending.insert(child);
            }
        }
        for (id, symbols) in imports {
            let provider = &self.providers[&id];
            let stub = scoop_toolchain::write_link_stub(
                &provider.install_name,
                provider.current_version,
                provider.compatibility_version,
                &symbols,
            )
            .map_err(error)?;
            self.stubs.insert(id, stub);
        }
        Ok(())
    }
}
