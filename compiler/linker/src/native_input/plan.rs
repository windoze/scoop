use super::*;

impl WireEncode for NativeInputs {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(4)?;
        e.array(self.libraries.len() as u64)?;
        for (id, library) in &self.libraries {
            e.array(5)?;
            id.encode(e)?;
            library.key.encode(e)?;
            e.array(library.inputs.len() as u64)?;
            for input in &library.inputs {
                input.encode(e)?;
            }
            e.array(library.scripts.len() as u64)?;
            for (path, digest) in &library.scripts {
                e.array(2)?;
                e.bytes(path.as_os_str().as_encoded_bytes())?;
                digest.encode(e)?;
            }
            e.unsigned(u64::from(library.system_alias))?;
        }
        e.array(self.files.len() as u64)?;
        for file in self.files.values() {
            e.array(4)?;
            file.id.encode(e)?;
            e.unsigned(file.slice.start as u64)?;
            e.unsigned(file.slice.end as u64)?;
            match &file.content {
                NativeContent::Dynamic(records) => {
                    e.array(records.len() as u64)?;
                    for record in records {
                        record.id.encode(e)?;
                    }
                }
                NativeContent::System | NativeContent::Object(_) | NativeContent::ElfDynamic(_) => {
                    e.array(0)?
                }
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
        let ordered = self.ordered_files();
        e.array(ordered.len() as u64)?;
        for file in ordered {
            file.id.encode(e)?;
        }
        Ok(())
    }
}

impl NativeInputs {
    pub fn dump(&self) -> String {
        let mut text = String::new();
        for (id, library) in &self.libraries {
            for input in &library.inputs {
                text.push_str(&format!(
                    "native library {} requirement={id} input={input} origins={}\n",
                    library.key.library().as_str(),
                    library.origins.join(", ")
                ));
            }
        }
        for file in self.ordered_files() {
            match &file.content {
                NativeContent::System
                | NativeContent::Dynamic(_)
                | NativeContent::ElfDynamic(_) => {
                    continue;
                }
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
