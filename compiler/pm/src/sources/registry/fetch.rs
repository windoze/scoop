use super::*;

#[derive(Clone, Debug)]
pub struct CachedPackage {
    pub root: PathBuf,
    pub manifest_text: String,
    pub document: PackageDocument,
    pub metadata: RegistryVersion,
}

#[derive(Serialize, Deserialize)]
struct PackageRecord {
    metadata: RegistryVersion,
    manifest_text: String,
    document: PackageDocument,
}

struct Locations {
    archive: PathBuf,
    record: PathBuf,
    source: PathBuf,
}

impl Locations {
    fn new(cache: &PackageCache, registry: &RegistryUrl, coordinate: &ConeCoordinate) -> Self {
        let directory = cache.registry(registry);
        let name = Path::new(coordinate.group())
            .join(coordinate.name())
            .join(coordinate.version());
        let archive = directory
            .join("cache")
            .join(&name)
            .with_file_name(format!("{}.tar.zst", coordinate.version()));
        let record = archive.with_file_name(format!("{}.json", coordinate.version()));
        Self {
            archive,
            record,
            source: directory.join("src").join(name),
        }
    }

    fn cached(&self, sha256: &str) -> PmResult<Option<CachedPackage>> {
        if !self
            .source
            .try_exists()
            .map_err(|error| PmError::io(&self.source, error))?
        {
            return Ok(None);
        }
        let record: PackageRecord = read_json(&self.record)?.ok_or_else(|| {
            PmError::new(format!("missing package record {}", self.record.display()))
        })?;
        if record.metadata.sha256 != sha256 {
            return Err(immutable_error(&self.source));
        }
        Ok(Some(CachedPackage {
            root: self.source.clone(),
            manifest_text: record.manifest_text,
            document: record.document,
            metadata: record.metadata,
        }))
    }
}

pub(crate) fn cached(
    cache: &PackageCache,
    registry: &RegistryUrl,
    coordinate: &ConeCoordinate,
    version: &RegistryVersion,
) -> PmResult<Option<CachedPackage>> {
    let locations = Locations::new(cache, registry, coordinate);
    let package = locations.cached(&version.sha256)?;
    if let Some(package) = &package {
        same_publication(&package.metadata, version, &locations.source)?;
    }
    Ok(package)
}

pub fn fetch_locked(
    client: &RegistryClient<'_>,
    registry: &RegistryUrl,
    coordinate: &ConeCoordinate,
    archive: &str,
    sha256: &str,
    offline: bool,
) -> PmResult<CachedPackage> {
    let cache = client.cache;
    let locations = Locations::new(cache, registry, coordinate);
    if let Some(package) = locations.cached(sha256)? {
        if package.metadata.archive != archive {
            return Err(immutable_error(&locations.source));
        }
        return Ok(package);
    }
    let index = load_index(
        client,
        registry,
        &DependencyCoordinateKey::from_coordinate(coordinate),
        offline,
        false,
    )?;
    let version = index
        .versions
        .iter()
        .find(|version| version.version == coordinate.version())
        .ok_or_else(|| {
            PmError::new(format!(
                "locked package {coordinate} is missing from {registry}"
            ))
        })?;
    if version.archive != archive || version.sha256 != sha256 {
        return Err(immutable_error(&locations.source));
    }
    fetch(client, registry, coordinate, version, offline)
}

pub fn fetch(
    client: &RegistryClient<'_>,
    registry: &RegistryUrl,
    coordinate: &ConeCoordinate,
    version: &RegistryVersion,
    offline: bool,
) -> PmResult<CachedPackage> {
    let cache = client.cache;
    let locations = Locations::new(cache, registry, coordinate);
    let _guard = cache.lock(
        "registry-archive",
        format!("{registry}/{coordinate}").as_bytes(),
    )?;
    if let Some(package) = locations.cached(&version.sha256)? {
        same_publication(&package.metadata, version, &locations.source)?;
        return Ok(package);
    }
    let archive_parent = locations.archive.parent().expect("archive has a parent");
    let source_parent = locations.source.parent().expect("package has a parent");
    fs::create_dir_all(archive_parent).map_err(|error| PmError::io(archive_parent, error))?;
    fs::create_dir_all(source_parent).map_err(|error| PmError::io(source_parent, error))?;
    let record = read_json::<PackageRecord>(&locations.record)?;
    if let Some(record) = &record {
        same_publication(&record.metadata, version, &locations.source)?;
    }
    let existing = record.is_some()
        && locations
            .archive
            .try_exists()
            .map_err(|error| PmError::io(&locations.archive, error))?;
    let download = if existing {
        None
    } else {
        if offline {
            return Err(PmError::new(format!(
                "archive for {coordinate} is not cached; fetch without --offline"
            )));
        }
        let file = tempfile::NamedTempFile::new_in(archive_parent)
            .map_err(|error| PmError::io(archive_parent, error))?;
        archive::download(
            &client.http,
            &archive_url(registry, &version.archive)?,
            file.path(),
            &version.sha256,
        )?;
        Some(file)
    };
    let staged =
        tempfile::tempdir_in(source_parent).map_err(|error| PmError::io(source_parent, error))?;
    archive::extract(
        download
            .as_ref()
            .map(|file| file.path())
            .unwrap_or(&locations.archive),
        staged.path(),
    )?;
    let manifest = staged.path().join("Cone.toml");
    let text = fs::read_to_string(&manifest).map_err(|error| PmError::io(&manifest, error))?;
    let (actual, kind, dependencies, document) = published_manifest(&text)?;
    let mut expected = version.dependencies.clone();
    expected.sort_by(|left, right| left.key.cmp(&right.key));
    if &actual != coordinate || kind != version.kind || dependencies != expected {
        return Err(PmError::new(format!(
            "archive manifest does not match registry metadata for {coordinate}"
        )));
    }
    if let Some(download) = download {
        download
            .persist(&locations.archive)
            .map_err(|error| PmError::io(&locations.archive, error.error))?;
    }
    let record = PackageRecord {
        metadata: version.clone(),
        manifest_text: text.clone(),
        document: document.clone(),
    };
    write_json(&locations.record, &record)?;
    fs::rename(staged.path(), &locations.source)
        .map_err(|error| PmError::io(&locations.source, error))?;
    Ok(CachedPackage {
        root: locations.source,
        manifest_text: text,
        document,
        metadata: version.clone(),
    })
}

fn same_publication(left: &RegistryVersion, right: &RegistryVersion, path: &Path) -> PmResult<()> {
    let mut left = left.clone();
    left.yanked = right.yanked;
    if &left != right {
        return Err(immutable_error(path));
    }
    Ok(())
}

fn immutable_error(path: &Path) -> PmError {
    PmError::new(format!(
        "immutable registry version metadata changed for {}; cannot overwrite the cached package",
        path.display()
    ))
}
