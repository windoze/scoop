use super::*;

impl WireEncode for LinuxFinalLinkProfile {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(8)?;
        e.field(1)?;
        self.startup.profile().contract().target().encode(e)?;
        e.field(2)?;
        self.startup.profile().contract().encode(e)?;
        e.field(3)?;
        self.compiler_digest.encode(e)?;
        e.field(4)?;
        e.text(&self.linker_version)?;
        e.field(5)?;
        e.array((OPTIONS.len() + self.mode_flags().len()) as u64)?;
        for option in OPTIONS.iter().chain(self.mode_flags()) {
            e.text(option)?;
        }
        e.field(6)?;
        sha256(self.metadata_script().as_bytes()).encode(e)?;
        e.field(7)?;
        // Locators serve local reads. Content and input role determine the
        // final-link identity, so moving a sysroot does not change its bytes.
        let mut inputs: Vec<_> = self
            .inputs
            .iter()
            .map(|input| {
                (
                    input.path.file_name().unwrap_or_default().to_string_lossy(),
                    input.digest,
                )
            })
            .collect();
        inputs.sort();
        e.array(inputs.len() as u64)?;
        for (name, digest) in inputs {
            e.array(2)?;
            e.text(&name)?;
            digest.encode(e)?;
        }
        e.field(8)?;
        e.unsigned(1)
    }
}
