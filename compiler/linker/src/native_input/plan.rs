use super::*;

impl WireEncode for NativeInputs {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(3)?;
        e.array(self.libraries.len() as u64)?;
        for (id, library) in &self.libraries {
            e.array(3)?;
            id.encode(e)?;
            library.key.encode(e)?;
            library.input.encode(e)?;
        }
        e.array(self.files.len() as u64)?;
        for file in self.files.values() {
            e.array(4)?;
            file.id.encode(e)?;
            e.unsigned(file.slice.start as u64)?;
            e.unsigned(file.slice.end as u64)?;
            match &file.content {
                NativeContent::Object(_) => e.array(0)?,
                NativeContent::Archive(members) => {
                    e.array(members.len() as u64)?;
                    for member in members {
                        e.array(3)?;
                        NativeObjectId::ArchiveMember(member.id).encode(e)?;
                        e.text(&member.name)?;
                        member.digest.encode(e)?;
                    }
                }
            }
        }
        e.array(self.selected.len() as u64)?;
        for id in self.selected.keys() {
            id.encode(e)?;
        }
        Ok(())
    }
}

impl NativeInputs {
    pub fn dump(&self) -> String {
        let mut text = String::new();
        for (id, library) in &self.libraries {
            text.push_str(&format!(
                "native library {} requirement={id} input={} origins={}\n",
                library.key.library().as_str(),
                library.input,
                library.origins.join(", ")
            ));
        }
        for file in self.ordered_files() {
            match &file.content {
                NativeContent::Object(_) => text.push_str(&format!(
                    "native object {} slice={}..{}\n",
                    file.id, file.slice.start, file.slice.end
                )),
                NativeContent::Archive(members) => {
                    text.push_str(&format!(
                        "native archive {} members={}\n",
                        file.id,
                        members.len()
                    ));
                    for member in members {
                        let selected = self
                            .selected
                            .contains_key(&NativeObjectId::ArchiveMember(member.id));
                        text.push_str(&format!("  member {} header={} payload={}..{} name={:?} digest={} selected={selected}\n", member.id.ordinal, member.id.header_offset, member.range.start, member.range.end, member.name, member.digest));
                    }
                }
            }
        }
        text
    }
}
