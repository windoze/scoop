use super::*;

mod source;

pub(super) struct Plan {
    pub sites: Vec<Site>,
}

pub(super) struct Site {
    pub prefix: String,
    pub name: String,
    pub manifest: PathBuf,
    pub graphs: Vec<ResolvedBuildGraph>,
    pub sources: BTreeMap<ConeIdentity, Source>,
    pub selected: BTreeSet<ConeIdentity>,
}

pub(super) struct Source {
    pub identity: ConeIdentity,
    pub key: String,
    pub root: PathBuf,
    pub readme: Option<PathBuf>,
    pub sources: Vec<SelectedSource>,
    pub dependencies: Vec<scoop_doc::ConeReference>,
}

impl Plan {
    pub fn new(
        mut graphs: Vec<ResolvedBuildGraph>,
        request: &DocumentRequest,
    ) -> BuildResult<Self> {
        if graphs.is_empty() {
            return Err(error("documentation requires at least one manifest root"));
        }
        let compiler = file_state(request.compiler.as_path())?.ok_or_else(|| {
            io_error(request.compiler.as_path(), "paired compiler does not exist")
        })?;
        // A consumer produces a shared source dependency and its documentation
        // together before an explicit root for that dependency is considered.
        graphs.sort_by_key(|graph| std::cmp::Reverse(graph.node_count()));
        let mut sites = Vec::new();
        let mut shared = BTreeMap::new();
        let mut compatible = true;
        let mut seen = BTreeSet::new();
        for graph in graphs {
            let root = graph.root_identity();
            let manifest = graph.source_manifest(root).unwrap();
            let key = input_key(&(
                root.to_string(),
                manifest.manifest_path().as_os_str().as_encoded_bytes(),
            ))?;
            if !seen.insert(key.clone()) {
                continue;
            }
            let sources = source::collect(&graph, request, &compiler)?;
            for (id, source) in &sources {
                if let Some(previous) = shared.insert(*id, source.key.clone()) {
                    compatible &= previous == source.key;
                }
            }
            let selected = sources
                .keys()
                .copied()
                .filter(|id| !request.no_deps || *id == root)
                .collect();
            sites.push(Site {
                prefix: format!("r/{key}"),
                name: manifest.coordinate().to_string(),
                manifest: manifest.manifest_path().to_owned(),
                graphs: vec![graph],
                sources,
                selected,
            });
        }
        if compatible {
            let mut combined = sites.remove(0);
            combined.prefix.clear();
            for site in sites {
                combined.graphs.extend(site.graphs);
                combined.sources.extend(site.sources);
                combined.selected.extend(site.selected);
            }
            sites = vec![combined];
        } else {
            sites.sort_by(|a, b| a.prefix.cmp(&b.prefix));
        }
        Ok(Self { sites })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct FileState {
    pub modified: std::time::SystemTime,
    pub length: u64,
}

pub(super) fn file_state(path: &Path) -> BuildResult<Option<FileState>> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(Some(FileState {
            modified: metadata.modified().map_err(|e| io_error(path, e))?,
            length: metadata.len(),
        })),
        Ok(_) => Err(io_error(path, "expected a regular file")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(path, e)),
    }
}
