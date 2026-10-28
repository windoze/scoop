use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use scoop_identity::{ConeCoordinate, ConeIdentity, TargetProfileId};
use scoop_lir::ValidatedLirTargetSelection;

use super::{ArtifactConeInput, LoadError, files::FileStamp, normalize_path};

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
}
