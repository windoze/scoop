use std::path::{Path, PathBuf};

use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::ProgramLinkClosure;
use scoop_toolchain::ValidatedFinalLinkProfile;

use crate::{LinkError, ProgramLinkOutput, RuntimeObjectSet, error, link_program};

pub struct ArtifactLinkRequest {
    pub root_slib: PathBuf,
    pub dependency_slibs: Vec<PathBuf>,
    pub runtime_index: PathBuf,
    pub target: String,
    pub library_paths: Vec<PathBuf>,
    pub output: PathBuf,
}

impl ArtifactLinkRequest {
    pub fn link(&self) -> Result<ProgramLinkOutput, LinkError> {
        let profile = ValidatedFinalLinkProfile::resolve(&self.target).map_err(error)?;
        let closure = read_program_artifacts(&self.root_slib, &self.dependency_slibs, &profile)?;
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
    profile: &ValidatedFinalLinkProfile,
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
    scoop_slib::read_program_link_closure(
        &root_bytes,
        &dependencies,
        ValidatedLirTargetSelection::from_id(profile.id()),
        profile.startup_toolchain().profile(),
    )
    .map_err(|err| error(format!("program Link input {}: {err}", root.display())))
}
