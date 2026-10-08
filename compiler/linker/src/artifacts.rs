use std::path::{Path, PathBuf};

use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::ProgramLinkClosure;
use scoop_toolchain::{FinalLinkOptions, ResolvedTargetProfile, ValidatedFinalLinkProfile};

use crate::{LinkError, ProgramLinkOutput, RuntimeObjectSet, error, link_program};

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests;

pub struct ArtifactLinkRequest {
    pub root_slib: PathBuf,
    pub dependency_slibs: Vec<PathBuf>,
    pub runtime_index: PathBuf,
    pub target: String,
    pub c_toolchain: scoop_toolchain::CToolchainOptions,
    pub final_link: scoop_toolchain::FinalLinkOptions,
    pub library_paths: Vec<PathBuf>,
    pub output: PathBuf,
}

impl ArtifactLinkRequest {
    pub fn link(&self) -> Result<ProgramLinkOutput, LinkError> {
        let target =
            ResolvedTargetProfile::resolve_with(&self.target, &self.c_toolchain).map_err(error)?;
        let closure = read_program_artifacts(
            &self.root_slib,
            &self.dependency_slibs,
            target.lir_target_selection(),
            target.c_bridge_toolchain().profile(),
        )?;
        let profile = resolve_program_link_profile(&closure, &target, &self.final_link)?;
        let runtime = RuntimeObjectSet::read_index(
            &self.runtime_index,
            profile.target(),
            profile.startup_toolchain().profile(),
        )?;
        link_program(
            &closure,
            &runtime,
            &profile,
            &self.library_paths,
            &self.output,
        )
    }
}

pub fn read_program_artifacts(
    root: &Path,
    dependencies: &[PathBuf],
    target: ValidatedLirTargetSelection,
    toolchain: &scoop_lir::CBridgeToolchainProfileV1,
) -> Result<ProgramLinkClosure, LinkError> {
    let read = |path: &Path| {
        std::fs::read(path).map_err(|err| error(format!("artifact {}: {err}", path.display())))
    };
    let root_bytes = read(root)?;
    let dependency_bytes = dependencies
        .iter()
        .map(|path| read(path))
        .collect::<Result<Vec<_>, _>>()?;
    let dependencies: Vec<_> = dependency_bytes.iter().map(Vec::as_slice).collect();
    scoop_slib::read_program_link_closure(&root_bytes, &dependencies, target, toolchain)
        .map_err(|err| error(format!("program Link input {}: {err}", root.display())))
}

pub fn resolve_program_link_profile(
    closure: &ProgramLinkClosure,
    target: &ResolvedTargetProfile,
    options: &FinalLinkOptions,
) -> Result<ValidatedFinalLinkProfile, LinkError> {
    let origins = closure
        .artifacts()
        .filter(|(artifact, _)| artifact.foundation().native_cxx())
        .map(|(artifact, _)| artifact.manifest().cone().coordinate().to_string())
        .collect::<Vec<_>>();
    if !origins.is_empty() && target.id() == scoop_lir::TargetProfileId::LinuxX86_64Musl {
        return Err(error(format!(
            "C++ native runtime is not supported for Linux musl; required by {}",
            origins.join(", ")
        )));
    }
    target
        .final_link_with_cxx(options, !origins.is_empty())
        .map_err(|err| {
            if origins.is_empty() {
                error(err)
            } else {
                error(format!("{err}; C++ required by {}", origins.join(", ")))
            }
        })
}
