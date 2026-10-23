use super::select::SelectedRoot;
use super::*;

pub(super) fn select(
    manifest: &LoadedConeManifest,
    roots: Vec<SelectedRoot>,
) -> Result<SelectedManifestSources, SourceDiscoveryError> {
    let mut candidates = Vec::new();
    let mut directories = Vec::new();
    for root in roots {
        let mut selected = Vec::new();
        if root.resolved.metadata.is_dir() {
            walk_directory(
                &root.resolved.path,
                root.path.as_path(),
                &root.selection,
                &mut BTreeSet::new(),
                &mut selected,
            )?;
            directories.push((root.path, root.role));
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
                metadata: root.resolved.metadata,
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
        let identity = SourceIdentity::new(manifest.identity(), candidate.logical_path);
        SelectedSource::new(
            identity,
            SourceDisplayLocator::new(candidate.display_path.clone()),
            candidate.physical_path,
            &candidate.metadata,
            role,
        )
        .map_err(|error| {
            SourceDiscoveryError::io(DiscoveryIoOperation::Inspect, candidate.display_path, error)
        })
    });
    let first = sources.next().expect("production sources are non-empty")?;
    let rest = sources.collect::<Result<Vec<_>, SourceDiscoveryError>>()?;
    Ok(SelectedManifestSources {
        first,
        rest,
        directories,
    })
}
