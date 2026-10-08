use super::*;

impl NativeInputs {
    pub fn include_embedded(
        &mut self,
        closure: &scoop_slib::ProgramLinkClosure,
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<(), LinkError> {
        for (artifact, _) in closure.artifacts() {
            for object in artifact.native_objects() {
                let file =
                    NativeInputId::from_bytes(object.bytes(), NativeFileKind::Object, profile)?;
                let id = NativeInputId(
                    domain_separated_cbor_hash(
                        "scoop-embedded-native-v1",
                        &EmbeddedKey {
                            member: object.member(),
                            file,
                        },
                    )
                    .map_err(error)?,
                );
                let locator = PathBuf::from(format!(
                    "{}:{}",
                    artifact.manifest().cone().coordinate(),
                    object.source().as_str()
                ));
                let index = NativeObjectIndex::read_with_toolchain(
                    object.bytes(),
                    profile.startup_toolchain().profile(),
                )
                .map_err(|err| error(format!("{}: {err}", locator.display())))?;
                self.files.insert(
                    id,
                    NativeFile {
                        id,
                        bytes: Arc::clone(object.bytes()),
                        slice: 0..object.bytes().len(),
                        locator,
                        content: NativeContent::Object(index),
                    },
                );
                self.embedded
                    .insert((artifact.identity(), object.member()), id);
            }
        }
        Ok(())
    }
}

struct EmbeddedKey {
    member: scoop_slib::SlibMemberId,
    file: NativeInputId,
}

impl WireEncode for EmbeddedKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(2)?;
        self.member.encode(encoder)?;
        self.file.encode(encoder)
    }
}
