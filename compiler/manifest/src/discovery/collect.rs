use super::select::SelectedRoot;
use super::*;

pub(super) fn discover(
    manifest: &LoadedConeManifest,
    roots: Vec<SelectedRoot>,
) -> Result<DiscoveredManifestSources, SourceDiscoveryError> {
    let mut candidates = Vec::new();
    let mut directories = Vec::new();
    for root in roots {
        let mut selected = Vec::new();
        if root.resolved.metadata.is_dir() {
            walk_directory(
                &root.resolved.path,
                root.path.as_path(),
                manifest.real_root(),
                &root.selection,
                &mut BTreeSet::new(),
                &mut selected,
            )?;
            directories.push(root.path);
        } else {
            let logical_path = NormalizedSourcePath::new(root.path.as_str()).map_err(|error| {
                SourceDiscoveryError::new(
                    root.resolved.path.clone(),
                    SourceDiscoveryErrorKind::InvalidLogicalPath(error),
                )
            })?;
            selected.push(SourceCandidate {
                logical_path,
                physical_path: root.resolved.path,
                display_path: manifest.real_root().join(root.path.as_path()),
                selection: root.selection,
            });
        }
        candidates.extend(selected.into_iter().map(|candidate| (root.role, candidate)));
    }
    if !candidates
        .iter()
        .any(|(role, _)| *role == SourceRole::Production)
    {
        return Err(SourceDiscoveryError::new(
            manifest.manifest_path().to_path_buf(),
            SourceDiscoveryErrorKind::EmptySourceSet,
        ));
    }
    candidates.sort_by(|(_, left), (_, right)| left.logical_path.cmp(&right.logical_path));
    let mut physical_sources: BTreeMap<&Path, &SourceCandidate> = BTreeMap::new();
    for (_, candidate) in &candidates {
        if let Some(first) = physical_sources.insert(&candidate.physical_path, candidate) {
            return Err(SourceDiscoveryError::new(
                candidate.display_path.clone(),
                SourceDiscoveryErrorKind::ConflictingSelections {
                    first: format!("{}: {}", first.selection, first.logical_path),
                    second: format!("{}: {}", candidate.selection, candidate.logical_path),
                },
            ));
        }
    }
    let mut sources = candidates.into_iter().map(|(role, candidate)| {
        let mut source = read_discovered_source(candidate, manifest.identity())?;
        source.role = role;
        Ok(source)
    });
    let first = sources.next().expect("production sources are non-empty")?;
    let rest = sources.collect::<Result<Vec<_>, SourceDiscoveryError>>()?;
    Ok(DiscoveredManifestSources {
        first,
        rest,
        directories,
    })
}
