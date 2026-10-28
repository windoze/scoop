use std::fs::Metadata;

use super::*;

/// The current location and metadata of one manifest input.
#[derive(Debug)]
pub struct LocatedConeManifest {
    real_root: PathBuf,
    manifest_path: PathBuf,
    metadata: Metadata,
}

impl ManifestRootLocator {
    pub fn resolve(&self) -> Result<LocatedConeManifest, ManifestRootError> {
        let (real_root, manifest_path) = match self {
            ManifestRootLocator::ConeDirectory(root) => {
                let real_root = canonicalize(root)?;
                if !std::fs::metadata(&real_root)
                    .map_err(|error| {
                        ManifestRootError::io(
                            ManifestRootIoOperation::Inspect,
                            real_root.clone(),
                            error,
                        )
                    })?
                    .is_dir()
                {
                    return Err(ManifestRootError::new(
                        root.clone(),
                        ManifestRootErrorKind::RootNotDirectory,
                    ));
                }
                let manifest_path = real_root.join(MANIFEST_FILE_NAME);
                (real_root, manifest_path)
            }
            ManifestRootLocator::ExactConeManifestFile(manifest) => {
                if !manifest
                    .file_name()
                    .is_some_and(|name| name == MANIFEST_FILE_NAME)
                {
                    return Err(ManifestRootError::new(
                        manifest.clone(),
                        ManifestRootErrorKind::InvalidManifestFileName,
                    ));
                }
                let manifest_path = canonicalize(manifest)?;
                let real_root = manifest_path.parent().ok_or_else(|| {
                    ManifestRootError::new(
                        manifest.clone(),
                        ManifestRootErrorKind::RootNotDirectory,
                    )
                })?;
                let real_root = canonicalize(real_root)?;
                (real_root, manifest_path)
            }
        };

        let metadata = std::fs::metadata(&manifest_path).map_err(|error| {
            ManifestRootError::io(
                ManifestRootIoOperation::Inspect,
                manifest_path.clone(),
                error,
            )
        })?;
        if !metadata.is_file() {
            return Err(ManifestRootError::new(
                manifest_path,
                ManifestRootErrorKind::ManifestNotRegularFile,
            ));
        }

        Ok(LocatedConeManifest {
            real_root,
            manifest_path,
            metadata,
        })
    }
}

impl LocatedConeManifest {
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    pub fn load(
        self,
        catalog: &[crate::SourceTarget<'_>],
    ) -> Result<LoadedConeManifest, ManifestRootError> {
        let manifest_path = &self.manifest_path;
        let bytes = std::fs::read(manifest_path).map_err(|error| {
            ManifestRootError::io(ManifestRootIoOperation::Read, manifest_path.clone(), error)
        })?;
        let source = String::from_utf8(bytes).map_err(|_| {
            ManifestRootError::new(manifest_path.clone(), ManifestRootErrorKind::InvalidUtf8)
        })?;
        let parsed = parse_cone_manifest(&source, catalog).map_err(|error| {
            ManifestRootError::new(manifest_path.clone(), ManifestRootErrorKind::Parse(error))
        })?;

        self.with_parsed(source, parsed)
    }

    /// Attach the parsed data retained by a build record for this input.
    pub fn with_parsed(
        self,
        source: String,
        parsed: ParsedConeManifest,
    ) -> Result<LoadedConeManifest, ManifestRootError> {
        let Self {
            real_root,
            manifest_path,
            ..
        } = self;
        LoadedConeManifest::from_expanded(
            real_root,
            manifest_path,
            source,
            parsed,
            std::collections::BTreeMap::new(),
        )
    }
}
