use super::*;
use scoop_wire::sha256;

impl DynamicInputs {
    pub fn encode(
        &self,
        bindings: &BTreeMap<String, DynamicBinding>,
        e: &mut Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(4)?;
        e.array(self.providers.len() as u64)?;
        for provider in self.providers.values() {
            e.array(8)?;
            provider.id.encode(e)?;
            provider.input.encode(e)?;
            e.text(&provider.install_name)?;
            e.unsigned(u64::from(provider.current_version))?;
            e.unsigned(u64::from(provider.compatibility_version))?;
            e.array(provider.dependencies.len() as u64)?;
            for (index, dependency) in provider.dependencies.iter().enumerate() {
                e.array(4)?;
                e.text(&dependency.name)?;
                e.unsigned(u64::from(dependency.reexport))?;
                e.unsigned(u64::from(dependency.compatibility_version))?;
                self.edges[&(provider.id, index)].encode(e)?;
            }
            e.array(provider.rpaths.len() as u64)?;
            for path in &provider.rpaths {
                e.text(path)?;
            }
            e.unsigned(u64::from(self.roots.contains(&provider.id)))?;
        }
        e.array(bindings.len() as u64)?;
        for (symbol, binding) in bindings {
            e.array(6)?;
            e.text(symbol)?;
            binding.owner.encode(e)?;
            binding.source.encode(e)?;
            e.text(&binding.source_symbol)?;
            e.unsigned(if binding.interface.kind == SystemExportKind::ThreadLocal {
                2
            } else {
                1
            })?;
            e.unsigned(u64::from(binding.interface.weak))?;
        }
        e.array(self.stubs.len() as u64)?;
        for (id, bytes) in &self.stubs {
            e.array(2)?;
            id.encode(e)?;
            sha256(bytes).encode(e)?;
        }
        e.array(self.rpaths.len() as u64)?;
        for path in &self.rpaths {
            e.text(path)?;
        }
        Ok(())
    }

    pub fn dump(&self, bindings: &BTreeMap<String, DynamicBinding>) -> String {
        let mut text = String::new();
        for provider in self.providers.values() {
            // Keep the established basic plan compact; native bindings below
            // still record their actual SDK owner in the canonical encoding.
            if provider.install_name == scoop_toolchain::LIBSYSTEM_INSTALL_NAME {
                continue;
            }
            text.push_str(&format!(
                "dynamic provider {} install={:?} imports={}\n",
                provider.id,
                provider.install_name,
                bindings
                    .values()
                    .filter(|binding| binding.owner == provider.id)
                    .count()
            ));
            for (index, edge) in provider.dependencies.iter().enumerate() {
                text.push_str(&format!(
                    "  {} {:?} -> {}\n",
                    if edge.reexport { "re-export" } else { "load" },
                    edge.name,
                    self.edges[&(provider.id, index)]
                ));
            }
        }
        for (symbol, binding) in bindings {
            if self.providers[&binding.owner].install_name
                != scoop_toolchain::LIBSYSTEM_INSTALL_NAME
            {
                text.push_str(&format!(
                    "dynamic binding {symbol} -> {:?} source={:?}:{} kind={:?} weak={}\n",
                    self.providers[&binding.owner].install_name,
                    self.providers[&binding.source].install_name,
                    binding.source_symbol,
                    binding.interface.kind,
                    binding.interface.weak
                ));
            }
        }
        for path in &self.rpaths {
            text.push_str(&format!("rpath {path:?}\n"));
        }
        text
    }
}
