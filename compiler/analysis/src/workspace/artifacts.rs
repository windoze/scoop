use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use scoop_identity::{ConeCoordinate, ConeIdentity, TargetProfileId};
use scoop_lir::ValidatedLirTargetSelection;

use super::{ArtifactConeInput, LoadError, ProjectConfiguration, files::FileStamp, normalize_path};

#[derive(Default)]
pub(super) struct ArtifactInputs {
    files: BTreeMap<(PathBuf, TargetProfileId), ArtifactConeInput>,
}

impl ArtifactInputs {
    pub(super) fn read(
        &mut self,
        path: &Path,
        coordinate: &ConeCoordinate,
        target: ValidatedLirTargetSelection,
    ) -> Result<ArtifactConeInput, LoadError> {
        let path = normalize_path(path).map_err(|error| LoadError::new(path, error))?;
        let stamp = FileStamp::read(&path)?;
        let key = (path.clone(), target.target().id());
        let input = if let Some(previous) = self.files.get(&key)
            && previous.stamp == stamp
        {
            previous.clone()
        } else {
            let file = scoop_slib::ArtifactFile::open(&path, target)
                .map_err(|error| LoadError::new(&path, error))?;
            let summary = Arc::new(
                file.summary()
                    .map_err(|error| LoadError::new(&path, error))?
                    .clone(),
            );
            let input = ArtifactConeInput {
                path: path.clone(),
                file,
                summary,
                stamp,
            };
            self.files.insert(key, input.clone());
            input
        };
        let cone = input.summary.cone();
        if cone.coordinate() != coordinate {
            return Err(LoadError::new(
                path,
                format!(
                    "dependency coordinate mismatch: expected {coordinate:?}, found {:?}",
                    cone.coordinate()
                ),
            ));
        }
        if cone.kind() != scoop_slib::ConeKind::Library
            || cone.source_form() != scoop_slib::ConeSourceForm::Manifest
            || cone.identity() == ConeIdentity::SINGLE_FILE
        {
            return Err(LoadError::new(
                path,
                "dependency must be a manifest library",
            ));
        }
        Ok(input)
    }

    pub(super) fn find(
        &mut self,
        coordinate: &ConeCoordinate,
        configuration: &ProjectConfiguration,
        default_core: bool,
    ) -> Result<Option<ArtifactConeInput>, LoadError> {
        let mut paths = Vec::new();
        if !default_core {
            for root in &configuration.artifact_search_roots {
                let path = scoop_toolchain::artifact_candidate_path(root, coordinate);
                if path
                    .try_exists()
                    .map_err(|error| LoadError::new(&path, error))?
                {
                    paths
                        .push(normalize_path(&path).map_err(|error| LoadError::new(&path, error))?);
                }
            }
        }
        if paths.is_empty()
            && (default_core
                || (coordinate.group() == "scoop"
                    && *coordinate != ConeCoordinate::reserved_core()))
        {
            paths.push(scoop_toolchain::sysroot_library_path(
                &configuration.sysroot,
                configuration.target.target(),
                coordinate,
            ));
        }
        paths.sort();
        paths.dedup();
        let mut selected: Option<ArtifactConeInput> = None;
        for path in paths {
            let input = self.read(&path, coordinate, configuration.target)?;
            if let Some(previous) = &selected {
                if input.summary.artifact_fingerprint() != previous.summary.artifact_fingerprint() {
                    return Err(LoadError::new(
                        path,
                        format!(
                            "ambiguous dependency artifact; differs from {}",
                            previous.path.display()
                        ),
                    ));
                }
            } else {
                selected = Some(input);
            }
        }
        if selected.is_none() && *coordinate != ConeCoordinate::reserved_core() {
            return Err(LoadError::new(
                &configuration.root,
                format!("dependency artifact was not found: {coordinate:?}"),
            ));
        }
        Ok(selected)
    }
}
