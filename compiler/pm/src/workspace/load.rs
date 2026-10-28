use super::*;

#[derive(Debug)]
pub(super) struct LoadedManifest {
    pub path: PathBuf,
    pub directory: PathBuf,
    pub input_directory: PathBuf,
    pub document: Arc<PackageDocument>,
    pub text: String,
}

pub(super) struct Manifests {
    loaded: BTreeMap<PathBuf, Arc<LoadedManifest>>,
    cache: Option<crate::cache::PackageCache>,
    overlays: ManifestOverlays,
    pub inputs: Vec<(PathBuf, crate::cache::FileStamp)>,
    pub missing_inputs: BTreeSet<PathBuf>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Record {
    text: String,
    document: PackageDocument,
}

impl Manifests {
    pub fn new(cache: Option<crate::cache::PackageCache>, overlays: ManifestOverlays) -> Self {
        Self {
            loaded: BTreeMap::new(),
            cache,
            overlays,
            inputs: Vec::new(),
            missing_inputs: BTreeSet::new(),
        }
    }

    pub fn load(&mut self, path: &Path) -> PmResult<Arc<LoadedManifest>> {
        let input_directory = std::path::absolute(path)
            .map_err(|error| PmError::io(path, error))?
            .parent()
            .expect("absolute manifest path has a parent")
            .to_path_buf();
        let canonical = self
            .canonicalize(path)
            .map_err(|error| PmError::io(path, error))?;
        if path != canonical && self.overlays.text(&canonical).is_none() {
            self.inputs
                .push((path.to_owned(), crate::cache::FileStamp::read(path)?));
        }
        let path = canonical;
        if let Some(loaded) = self.loaded.get(&path) {
            return Ok(loaded.clone());
        }
        if let Some(text) = self.overlays.text(&path) {
            let record = Record::parse(&path, text.to_owned())?;
            return Ok(self.insert(path, input_directory, record));
        }
        let stamp = crate::cache::FileStamp::read(&path)?;
        let record_path = self
            .cache
            .as_ref()
            .map(|cache| cache.input_record("manifests-v2", &path, &stamp, &[]))
            .transpose()?;
        let record = record_path.as_ref().map(std::fs::read).transpose();
        let record = match record {
            Ok(Some(bytes)) => Some(
                serde_json::from_slice::<Record>(&bytes)
                    .map_err(|error| PmError::new(error.to_string()))?,
            ),
            Ok(None) => None,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(PmError::new(error.to_string())),
        };
        let record = match record {
            Some(record) => record,
            None => {
                let text =
                    std::fs::read_to_string(&path).map_err(|error| PmError::io(&path, error))?;
                let record = Record::parse(&path, text)?;
                stamp.unchanged(&path)?;
                if let Some(path) = record_path {
                    let bytes = serde_json::to_vec(&record)
                        .map_err(|error| PmError::new(error.to_string()))?;
                    crate::cache::atomic_write(&path, &bytes)?;
                }
                record
            }
        };
        self.inputs.push((path.clone(), stamp));
        Ok(self.insert(path, input_directory, record))
    }

    fn insert(
        &mut self,
        path: PathBuf,
        input_directory: PathBuf,
        record: Record,
    ) -> Arc<LoadedManifest> {
        let loaded = Arc::new(LoadedManifest {
            input_directory,
            directory: path
                .parent()
                .expect("absolute manifest path has a parent")
                .to_path_buf(),
            path: path.clone(),
            document: Arc::new(record.document),
            text: record.text,
        });
        self.loaded.insert(path, loaded.clone());
        loaded
    }

    pub fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
        self.overlays.canonicalize(path)
    }

    pub fn overlay_exists(&self, path: &Path) -> bool {
        self.overlays.contains(path)
    }

    pub fn exists(&mut self, path: &Path) -> PmResult<bool> {
        if self.overlay_exists(path) || crate::cache::FileStamp::optional(path)?.is_some() {
            Ok(true)
        } else {
            self.missing_inputs.insert(path.to_owned());
            Ok(false)
        }
    }
}

impl Record {
    fn parse(path: &Path, text: String) -> PmResult<Self> {
        let document =
            scoop_manifest::package::parse_package_manifest(&text, scoop_toolchain::SOURCE_TARGETS)
                .map_err(|error| {
                    let mut result = PmError::new(format!("{}: {error}", path.display()));
                    if let Some(span) = error.span() {
                        result.origins.push(ManifestOrigin {
                            manifest: path.to_owned(),
                            span: span.range(),
                        });
                    }
                    result
                })?;
        Ok(Self { text, document })
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
