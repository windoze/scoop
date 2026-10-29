use super::*;

impl WireEncode for WindowsFinalLinkProfile {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(8)?;
        self.startup.target().wire_id().encode(e)?;
        self.startup.profile().contract().encode(e)?;
        self.data.compiler_digest.encode(e)?;
        self.data.linker_digest.encode(e)?;
        e.text(&self.data.linker_version)?;
        e.array(self.linker_args().len() as u64)?;
        for option in self.linker_args() {
            e.text(option)?;
        }
        let mut inputs: Vec<_> = self
            .data
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
        match &self.data.cxx {
            Some(cxx) => {
                e.array(1)?;
                cxx.fingerprint().encode(e)
            }
            None => e.array(0),
        }
    }
}
