use super::*;

pub(super) fn collect(
    graph: &ResolvedBuildGraph,
    request: &DocumentRequest,
    compiler: &FileState,
) -> BuildResult<BTreeMap<ConeIdentity, Source>> {
    let selection =
        scoop_toolchain::select_target(request.target.canonical_triple()).map_err(error)?;
    let mut sources = BTreeMap::new();
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
        let selected = scoop_manifest::select_manifest_sources(manifest, selection.target().id())
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
            manifest.input_root().as_os_str().as_encoded_bytes(),
            manifest.parsed().semantic(),
            input_sources,
            dependencies,
            request.compiler.as_path().as_os_str().as_encoded_bytes(),
            compiler,
            scoop_protocol::PROTOCOL_VERSION,
        ))?;
        keys.insert(*identity, key.clone());
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
    Ok(sources)
}
