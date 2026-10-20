use super::*;

#[derive(Deserialize, Serialize)]
pub(super) struct Record {
    pub key: String,
    pub files: Vec<String>,
    #[serde(default)]
    pub shared: Option<Pages>,
    #[serde(default)]
    pub cones: BTreeMap<String, ConePages>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct ConePages {
    pub api: Pages,
    pub overview: Pages,
}

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Pages {
    pub key: String,
    pub files: Vec<String>,
}

pub(super) fn load(output: &Path) -> BuildResult<Option<Record>> {
    let path = output.join(RECORD);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| io_error(&path, e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(&path, e)),
    }
}

pub(super) fn complete(output: &Path, files: &[String]) -> BuildResult<bool> {
    for file in files {
        let path = output.join(file);
        if !path.try_exists().map_err(|e| io_error(&path, e))? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn reuse(
    key: &str,
    previous: Option<&Pages>,
    output: &Path,
    stage: &Path,
) -> BuildResult<Option<Pages>> {
    let Some(previous) = previous.filter(|pages| pages.key == key) else {
        return Ok(None);
    };
    if !complete(output, &previous.files)? {
        return Ok(None);
    }
    for file in &previous.files {
        let path = stage.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| io_error(&path, e))?;
        std::fs::hard_link(output.join(file), &path).map_err(|e| io_error(&path, e))?;
    }
    Ok(Some(previous.clone()))
}

pub(super) fn render(
    stage: &Path,
    key: String,
    files: Vec<scoop_doc::RenderedFile>,
    rendered: &mut Vec<String>,
) -> BuildResult<Pages> {
    let mut paths = Vec::new();
    for file in files {
        write(stage, &file.path, &file.contents)?;
        rendered.push(file.path.clone());
        paths.push(file.path);
    }
    Ok(Pages { key, files: paths })
}

pub(super) fn resources(
    stage: &Path,
    overview: &overview::Overview,
    pages: &mut Pages,
) -> BuildResult<()> {
    for (source, destination) in &overview.copies {
        let path = stage.join(destination);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| io_error(&path, e))?;
        std::fs::copy(source, &path).map_err(|e| io_error(source, e))?;
        pages.files.push(destination.clone());
    }
    pages.files.sort();
    pages.files.dedup();
    Ok(())
}
