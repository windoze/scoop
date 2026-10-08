use super::*;

pub(super) struct SelectedRoot {
    pub path: ConeRelativePath,
    pub selection: String,
    pub resolved: ResolvedEntry,
}

pub(super) fn selected_roots(
    manifest: &LoadedConeManifest,
    target: TargetProfileId,
) -> Result<Vec<SelectedRoot>, SourceDiscoveryError> {
    let selection = manifest.parsed().semantic().sources();
    let paths: Vec<_> = match selection {
        SourceSelection::Default => vec![(
            ConeRelativePath::new("src").expect("the default source path is relative"),
            "default src/".to_owned(),
        )],
        SourceSelection::Explicit(paths) => paths
            .iter()
            .enumerate()
            .filter(|(_, path)| path.predicate().matches(target))
            .map(|(index, path)| {
                (
                    path.path().clone(),
                    format!("sources[{}] ({:?})", index + 1, path.path().as_str()),
                )
            })
            .collect(),
    };
    let mut roots: Vec<SelectedRoot> = Vec::new();
    for (path, label) in paths {
        let display_path = manifest.real_root().join(path.as_path());
        let resolved = resolve_entry(&display_path)?;
        if !resolved.path.starts_with(manifest.real_root()) {
            return Err(SourceDiscoveryError::new(
                display_path,
                SourceDiscoveryErrorKind::EscapesConeRoot,
            ));
        }
        if matches!(selection, SourceSelection::Default) && !resolved.metadata.is_dir() {
            return Err(SourceDiscoveryError::new(
                display_path,
                SourceDiscoveryErrorKind::SourceRootNotDirectory,
            ));
        }
        if !resolved.metadata.is_dir()
            && !(resolved.metadata.is_file()
                && path.as_path().extension() == Some(OsStr::new("scoop")))
        {
            return Err(SourceDiscoveryError::new(
                display_path,
                SourceDiscoveryErrorKind::SelectedPathNotSource,
            ));
        }
        let root = SelectedRoot {
            path,
            selection: label,
            resolved,
        };
        for prior in &roots {
            if overlaps(
                prior.path.as_path(),
                &prior.resolved.metadata,
                root.path.as_path(),
                &root.resolved.metadata,
            ) || overlaps(
                &prior.resolved.path,
                &prior.resolved.metadata,
                &root.resolved.path,
                &root.resolved.metadata,
            ) {
                return Err(SourceDiscoveryError::new(
                    display_path,
                    SourceDiscoveryErrorKind::ConflictingSelections {
                        first: prior.selection.clone(),
                        second: root.selection,
                    },
                ));
            }
        }
        roots.push(root);
    }
    Ok(roots)
}

fn overlaps(
    left: &Path,
    left_metadata: &std::fs::Metadata,
    right: &Path,
    right_metadata: &std::fs::Metadata,
) -> bool {
    left == right
        || (left_metadata.is_dir() && right.starts_with(left))
        || (right_metadata.is_dir() && left.starts_with(right))
}
