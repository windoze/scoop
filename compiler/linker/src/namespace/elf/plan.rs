use super::*;
use scoop_wire::WireEncode;

impl ElfNamespace {
    pub fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(4)?;
        e.unsigned(2)?;
        e.array(self.bindings.len() as u64)?;
        for (symbol, binding) in &self.bindings {
            e.array(2)?;
            e.text(symbol)?;
            match binding {
                ElfBinding::System(_) => e.array(0)?,
                ElfBinding::Shared { input, export } => {
                    e.array(2)?;
                    input.encode(e)?;
                    match &export.version {
                        Some(version) => {
                            e.array(1)?;
                            e.text(version)?;
                        }
                        None => e.array(0)?,
                    }
                }
            }
        }
        e.array(self.selected.len() as u64)?;
        for id in &self.selected {
            e.array(2)?;
            id.encode(e)?;
            e.text(&self.providers[id].name)?;
        }
        e.array(self.rpaths.len() as u64)?;
        for path in &self.rpaths {
            e.bytes(path.as_os_str().as_encoded_bytes())?;
        }
        Ok(())
    }

    pub fn dump(&self) -> String {
        let mut text = format!(
            "ELF system imports={}\n",
            self.bindings
                .values()
                .filter(|b| matches!(b, ElfBinding::System(_)))
                .count()
        );
        for id in &self.selected {
            text.push_str(&format!(
                "ELF shared library {id} name={:?}\n",
                self.providers[id].name
            ));
        }
        for (symbol, binding) in &self.bindings {
            if let ElfBinding::Shared { input, export } = binding {
                text.push_str(&format!(
                    "ELF binding {symbol} -> {} version={:?}\n",
                    self.providers[input].name, export.version
                ));
            }
        }
        text
    }
}
