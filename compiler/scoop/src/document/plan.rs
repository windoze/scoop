use super::*;

pub(super) struct Plan {
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
        let selection =
            scoop_toolchain::select_target(request.target.canonical_triple()).map_err(error)?;
        let compiler = file_state(request.compiler.as_path())?.ok_or_else(|| {
            io_error(request.compiler.as_path(), "paired compiler does not exist")
        })?;
        // A consumer produces a shared source dependency and its documentation
        // together before an explicit root for that dependency is considered.
        graphs.sort_by_key(|graph| std::cmp::Reverse(graph.node_count()));
        let roots = graphs
            .iter()
            .map(ResolvedBuildGraph::root_identity)
            .collect::<BTreeSet<_>>();
        let mut sources = BTreeMap::<ConeIdentity, Source>::new();
        for graph in &graphs {
            let mut keys = BTreeMap::new();
            for identity in graph.dependency_first() {
                let Some(manifest) = graph.source_manifest(*identity) else {
                    keys.insert(
                        *identity,
                        graph
                            .prebuilt_fingerprint(*identity)
                            .expect("manifest graphs contain source or prebuilt nodes")
                            .to_string(),
                    );
                    continue;
                };
                let selected =
                    scoop_manifest::select_manifest_sources(manifest, selection.target().id())
                        .map_err(|failure| io_error(failure.path(), &failure))?;
                let input_sources = selected
                    .iter()
                    .map(|source| {
                        (
                            source.identity().logical_path().as_str(),
                            source.physical_path().as_os_str().as_encoded_bytes(),
                            source.modified(),
                            source.byte_length(),
                        )
                    })
                    .collect::<Vec<_>>();
                let dependencies = graph
                    .edges()
                    .filter(|edge| edge.dependent() == *identity)
                    .map(|edge| (edge.dependency().to_string(), &keys[&edge.dependency()]))
                    .collect::<Vec<_>>();
                let key = input_key(&(
                    selection.target().contract().canonical_triple(),
                    manifest.manifest_path().as_os_str().as_encoded_bytes(),
                    manifest.parsed().semantic(),
                    input_sources,
                    dependencies,
                    request.compiler.as_path().as_os_str().as_encoded_bytes(),
                    &compiler,
                    scoop_protocol::PROTOCOL_VERSION,
                ))?;
                keys.insert(*identity, key.clone());
                if let Some(existing) = sources.get(identity) {
                    if existing.key != key {
                        return Err(error(format!(
                            "conflicting source or dependency inputs for {}",
                            manifest.coordinate()
                        )));
                    }
                    continue;
                }
                let dependencies = graph
                    .edges()
                    .filter(|edge| edge.dependent() == *identity)
                    .map(|edge| {
                        Ok(scoop_doc::ConeReference {
                            key: scoop_doc::ConeKey::new(edge.dependency()).map_err(error)?,
                            name: graph
                                .node_coordinate(edge.dependency())
                                .unwrap()
                                .to_string(),
                        })
                    })
                    .collect::<BuildResult<_>>()?;
                sources.insert(
                    *identity,
                    Source {
                        identity: *identity,
                        key,
                        root: manifest.real_root().to_owned(),
                        readme: manifest
                            .metadata()
                            .get("readme")
                            .map(|path| manifest.real_root().join(path)),
                        sources: selected.iter().cloned().collect(),
                        dependencies,
                    },
                );
            }
        }
        let selected = sources
            .keys()
            .copied()
            .filter(|id| !request.no_deps || roots.contains(id))
            .collect();
        Ok(Self {
            graphs,
            sources,
            selected,
        })
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
