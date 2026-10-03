use super::*;
use crate::native_input::NativeContent;

impl DynamicInputs {
    pub fn candidates(
        &self,
        native: &NativeInputs,
        symbol: &str,
        input: Option<NativeInputId>,
    ) -> Result<Vec<DynamicBinding>, LinkError> {
        let roots: Vec<_> = if let Some(input) = input {
            match &native.files[&input].content {
                NativeContent::Dynamic(records) => records
                    .first()
                    .map(|provider| vec![provider.id])
                    .unwrap_or_default(),
                _ => Vec::new(),
            }
        } else {
            self.roots.iter().copied().collect()
        };
        let mut result = Vec::new();
        for root in roots {
            if let Some(mut binding) = self.find(root, symbol, &mut BTreeSet::new())? {
                binding.owner = root;
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
                    self.find(*child, imported, visited)
                }
            };
        }
        for (index, dependency) in provider.dependencies.iter().enumerate() {
            if dependency.reexport
                && let Some(binding) = self.find(self.edges[&(id, index)], symbol, visited)?
            {
                return Ok(Some(binding));
            }
        }
        Ok(None)
    }
}
