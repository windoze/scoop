use super::*;

#[derive(Debug)]
pub(super) struct LoadedManifest {
    pub path: PathBuf,
    pub directory: PathBuf,
    pub document: Arc<PackageDocument>,
}

#[derive(Default)]
pub(super) struct Manifests(BTreeMap<PathBuf, Arc<LoadedManifest>>);

impl Manifests {
    pub fn load(&mut self, path: &Path) -> PmResult<Arc<LoadedManifest>> {
        let path = std::fs::canonicalize(path).map_err(|error| PmError::io(path, error))?;
        if let Some(loaded) = self.0.get(&path) {
            return Ok(loaded.clone());
        }
        let text = std::fs::read_to_string(&path).map_err(|error| PmError::io(&path, error))?;
        let document =
            scoop_manifest::package::parse_package_manifest(&text, scoop_toolchain::SOURCE_TARGETS)
                .map_err(|error| {
                    let mut result = PmError::new(format!("{}: {error}", path.display()));
                    if let Some(span) = error.span() {
                        result.origins.push(ManifestOrigin {
                            manifest: path.clone(),
                            span: span.range(),
                        });
                    }
                    result
                })?;
        let loaded = Arc::new(LoadedManifest {
            directory: path
                .parent()
                .expect("absolute manifest path has a parent")
                .to_path_buf(),
            path: path.clone(),
            document: Arc::new(document),
        });
        self.0.insert(path, loaded.clone());
        Ok(loaded)
    }
}

impl LoadedManifest {
    pub fn origin(&self, span: std::ops::Range<usize>) -> ManifestOrigin {
        ManifestOrigin {
            manifest: self.path.clone(),
            span,
        }
    }
}
