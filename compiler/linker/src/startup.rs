use scoop_process::CommandExt;
use std::collections::BTreeSet;
use std::path::Path;

use scoop_toolchain::ValidatedFinalLinkProfile;

use crate::{LinkError, NativeObjectInfo, NativeSymbolKind, error, program::ProgramInputs};

mod images;

pub(crate) struct StartupObject {
    pub source: String,
    pub bytes: Vec<u8>,
    pub references: BTreeSet<String>,
}

impl StartupObject {
    pub fn build(
        inputs: &ProgramInputs<'_>,
        profile: &ValidatedFinalLinkProfile,
        enable_stack_trace: bool,
        directory: &Path,
    ) -> Result<Self, LinkError> {
        let mut source = String::from("#include <stddef.h>\n#include <stdint.h>\n");
        source.push_str(images::TYPES);
        let section = match profile {
            ValidatedFinalLinkProfile::Darwin(_) => "__DATA_CONST,__const",
            ValidatedFinalLinkProfile::Linux(_) => ".data.rel.ro.scoop.startup",
            ValidatedFinalLinkProfile::Windows(_) => ".rdata$scoop_startup",
        };
        images::emit(&mut source, &inputs.final_images, section);
        Self::build_source(
            source,
            inputs.images.len(),
            &inputs.root,
            inputs
                .final_images
                .iter()
                .flat_map(|image| image.references().cloned())
                .collect(),
            inputs
                .final_images
                .iter()
                .flat_map(|image| {
                    std::iter::once(image.name.clone())
                        .chain(image.traps.iter().map(|(name, _)| name.clone()))
                })
                .collect(),
            profile,
            enable_stack_trace,
            directory,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build_source(
        mut source: String,
        image_count: usize,
        root: &str,
        mut references: BTreeSet<String>,
        definitions: BTreeSet<String>,
        profile: &ValidatedFinalLinkProfile,
        enable_stack_trace: bool,
        directory: &Path,
    ) -> Result<Self, LinkError> {
        source.push_str(&format!(
            "extern const ScoopRootEntryDescriptorV1 root_entry __asm__(\"{}\") __attribute__((visibility(\"hidden\")));\n",
            root
        ));
        let section = match profile {
            ValidatedFinalLinkProfile::Darwin(_) => "__DATA_CONST,__const",
            ValidatedFinalLinkProfile::Linux(_) => ".data.rel.ro.scoop.startup",
            ValidatedFinalLinkProfile::Windows(_) => ".rdata$scoop_startup",
        };
        let windows = matches!(profile, ValidatedFinalLinkProfile::Windows(_));
        let (entry, runtime, argument_type) = if windows {
            ("wmain", "scoop_rt_run_program_utf16", "uint16_t")
        } else {
            ("main", "scoop_rt_run_program", "char")
        };
        source.push_str(&format!("extern int {runtime}(const ScoopImageDescriptorV1 *const *, uint64_t, const ScoopRootEntryDescriptorV1 *, int32_t, const {argument_type} *const *, uint32_t);\n\n__attribute__((used, section(\"{section}\")))\nstatic const ScoopImageDescriptorV1 *const scoop_program_images[] = {{\n"));
        for index in 0..image_count {
            source.push_str(&format!("    &image_{index},\n"));
        }
        source.push_str(&format!("}};\n\nint {entry}(int argc, {argument_type} **argv) {{\n    return {runtime}(scoop_program_images, sizeof(scoop_program_images) / sizeof(scoop_program_images[0]), &root_entry, argc, (const {argument_type} *const *)argv, {});\n}}\n", u32::from(enable_stack_trace)));
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
        let mut info =
            NativeObjectInfo::read_with_toolchain(&bytes, profile.startup_toolchain().profile())?;
        if matches!(profile, ValidatedFinalLinkProfile::Linux(_)) {
            // GNU as may retain this implicit linker symbol without a use.
            info.requirements.remove("_GLOBAL_OFFSET_TABLE_");
        }
        let normalization = profile.target().contract().native_symbol_normalization();
        let main = normalization.compiler_generated_object_symbol(entry);
        references.extend([
            root.to_owned(),
            normalization.compiler_generated_object_symbol(runtime),
        ]);
        if info.requirements != references
            || info.definitions.len() != 1 + definitions.len()
            || definitions.iter().any(|name| {
                !info.definitions.get(name).is_some_and(|definition| {
                    definition.kind == NativeSymbolKind::Data
                        && definition.binding == crate::NativeSymbolBinding::Strong
                })
            })
            || !info.definitions.get(&main).is_some_and(|definition| {
                definition.kind == NativeSymbolKind::Function
                    && definition.binding == crate::NativeSymbolBinding::Strong
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

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests;
