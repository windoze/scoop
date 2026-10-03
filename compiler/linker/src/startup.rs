use scoop_process::CommandExt;
use std::collections::BTreeSet;
use std::path::Path;

use scoop_toolchain::ValidatedFinalLinkProfile;

use crate::{LinkError, NativeObjectInfo, NativeSymbolKind, error, program::ProgramInputs};

pub(crate) const IMAGE_ARRAY: &str = "_scoop_program_images";
pub(crate) struct StartupObject {
    pub source: String,
    pub bytes: Vec<u8>,
    pub references: BTreeSet<String>,
}

impl StartupObject {
    pub fn build(
        inputs: &ProgramInputs<'_>,
        profile: &ValidatedFinalLinkProfile,
        directory: &Path,
    ) -> Result<Self, LinkError> {
        let mut source = String::from(
            "#include <stdint.h>\n\ntypedef struct ScoopImageDescriptorV1 ScoopImageDescriptorV1;\ntypedef struct ScoopRootEntryDescriptorV1 ScoopRootEntryDescriptorV1;\n\n",
        );
        for (index, image) in inputs.images.iter().enumerate() {
            source.push_str(&format!(
                "extern const ScoopImageDescriptorV1 image_{index} __asm__(\"{image}\");\n"
            ));
        }
        source.push_str(&format!(
            "extern const ScoopRootEntryDescriptorV1 root_entry __asm__(\"{}\");\n",
            inputs.root
        ));
        source.push_str("extern int scoop_rt_run_program(const ScoopImageDescriptorV1 *const *, uint64_t, const ScoopRootEntryDescriptorV1 *);\n\n__attribute__((used, section(\"__DATA_CONST,__const\")))\nstatic const ScoopImageDescriptorV1 *const scoop_program_images[] = {\n");
        for index in 0..inputs.images.len() {
            source.push_str(&format!("    &image_{index},\n"));
        }
        source.push_str("};\n\nint main(void) {\n    return scoop_rt_run_program(scoop_program_images, sizeof(scoop_program_images) / sizeof(scoop_program_images[0]), &root_entry);\n}\n");
        let source_path = directory.join("startup.c");
        let object = directory.join("startup-compiled.o");
        std::fs::write(&source_path, &source).map_err(error)?;
        let output = profile
            .startup_toolchain()
            .object_compilation_command(&source_path, &object)
            .scoop_output()
            .map_err(error)?;
        if !output.status.success() {
            return Err(error(format!(
                "startup C compilation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let bytes = std::fs::read(object).map_err(error)?;
        let info = NativeObjectInfo::read(
            &bytes,
            profile
                .startup_toolchain()
                .profile()
                .contract()
                .deployment(),
        )?;
        let mut expected: BTreeSet<_> = inputs.images.iter().cloned().collect();
        expected.extend([inputs.root.clone(), "_scoop_rt_run_program".to_owned()]);
        if info.requirements != expected
            || info.definitions.len() != 1
            || !info.definitions.get("_main").is_some_and(|definition| {
                definition.kind == NativeSymbolKind::Function && !definition.weak
            })
        {
            return Err(error(
                "startup object definitions or image/root/runtime references differ from its plan",
            ));
        }
        Ok(Self {
            source,
            bytes,
            references: info.requirements,
        })
    }
}
