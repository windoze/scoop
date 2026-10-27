use super::*;

#[derive(Debug)]
pub(super) struct LoadedManifest {
    pub path: PathBuf,
    pub directory: PathBuf,
    pub document: Arc<PackageDocument>,
    pub text: String,
}

pub(super) struct Manifests {
    loaded: BTreeMap<PathBuf, Arc<LoadedManifest>>,
    cache: Option<crate::cache::PackageCache>,
    pub inputs: Vec<(PathBuf, crate::cache::FileStamp)>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Record {
    text: String,
    document: PackageDocument,
}

impl Manifests {
    pub fn new(cache: Option<crate::cache::PackageCache>) -> Self {
        Self {
            loaded: BTreeMap::new(),
            cache,
            inputs: Vec::new(),
        }
    }

    pub fn load(&mut self, path: &Path) -> PmResult<Arc<LoadedManifest>> {
        let path = std::fs::canonicalize(path).map_err(|error| PmError::io(path, error))?;
        if let Some(loaded) = self.loaded.get(&path) {
            return Ok(loaded.clone());
        }
        let stamp = crate::cache::FileStamp::read(&path)?;
        let record_path = self
            .cache
            .as_ref()
            .map(|cache| cache.input_record("manifests-v1", &path, &stamp, &[]))
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
                let document = scoop_manifest::package::parse_package_manifest(
                    &text,
                    scoop_toolchain::SOURCE_TARGETS,
                )
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
                stamp.unchanged(&path)?;
                let record = Record { text, document };
                if let Some(path) = record_path {
                    let bytes = serde_json::to_vec(&record)
                        .map_err(|error| PmError::new(error.to_string()))?;
                    crate::cache::atomic_write(&path, &bytes)?;
                }
                record
            }
        };
        self.inputs.push((path.clone(), stamp));
        let loaded = Arc::new(LoadedManifest {
            directory: path
                .parent()
                .expect("absolute manifest path has a parent")
                .to_path_buf(),
            path: path.clone(),
            document: Arc::new(record.document),
            text: record.text,
        });
        self.loaded.insert(path, loaded.clone());
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
