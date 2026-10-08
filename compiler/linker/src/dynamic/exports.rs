use super::*;
use crate::native_input::NativeContent;

impl DynamicInputs {
    pub fn candidates(
        &self,
        native: &NativeInputs,
        symbol: &str,
        input: Option<&[NativeInputId]>,
    ) -> Result<Vec<DynamicBinding>, LinkError> {
        let roots: Vec<_> = if let Some(inputs) = input {
            inputs
                .iter()
                .filter_map(|input| match &native.files[input].content {
                    NativeContent::Dynamic(records) => records.first().map(|provider| provider.id),
                    _ => None,
                })
                .collect()
        } else {
            self.roots.iter().copied().collect()
        };
        let mut result = Vec::new();
        for root in roots {
            if let Some(binding) = self.find(root, symbol, &mut BTreeSet::new())? {
                result.push(binding);
            }
        }
        Ok(result)
    }

    fn find(
        &self,
        id: NativeDynamicProviderId,
        symbol: &str,
        visited: &mut BTreeSet<(NativeDynamicProviderId, String)>,
    ) -> Result<Option<DynamicBinding>, LinkError> {
        if !visited.insert((id, symbol.into())) {
            return Ok(None);
        }
        let provider = &self.providers[&id];
        if let Some(export) = provider.exports.get(symbol) {
            return match export {
                DynamicExport::Symbol { interface, storage } => Ok(Some(DynamicBinding {
                    owner: id,
                    source: id,
                    source_symbol: symbol.into(),
                    interface: *interface,
                    storage: *storage,
                })),
                DynamicExport::Reexport {
                    dependency,
                    imported,
                } => {
                    let child = self.edges.get(&(id, *dependency)).ok_or_else(|| {
                        error(format!(
                            "native renamed re-export {symbol} has no resolved dependency"
                        ))
                    })?;
                    self.find(*child, imported, visited).map(|binding| {
                        binding.map(|mut binding| {
                            if binding.owner == *child {
                                binding.owner = id;
                            }
                            binding
                        })
                    })
                }
                DynamicExport::Previous {
                    dependency,
                    interface,
                } => {
                    let child = *self.edges.get(&(id, *dependency)).ok_or_else(|| {
                        error(format!("previous SDK export {symbol} has no load provider"))
                    })?;
                    Ok(Some(DynamicBinding {
                        owner: child,
                        source: child,
                        source_symbol: symbol.to_owned(),
                        interface: *interface,
                        storage: ExportStorage::InterfaceOnly,
                    }))
                }
            };
        }
        for (index, dependency) in provider.dependencies.iter().enumerate() {
            if dependency.reexport
                && let Some(mut binding) = self.find(self.edges[&(id, index)], symbol, visited)?
            {
                if binding.owner == self.edges[&(id, index)] {
                    binding.owner = id;
                }
                return Ok(Some(binding));
            }
        }
        Ok(None)
    }
}
