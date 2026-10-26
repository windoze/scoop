use super::*;

pub(super) struct SelectedRoot {
    pub path: ConeRelativePath,
    pub selection: String,
    pub resolved: ResolvedEntry,
    pub role: SourceRole,
}

pub(super) fn selected_roots(
    manifest: &LoadedConeManifest,
    target: TargetProfileId,
    include_tests: bool,
) -> Result<Vec<SelectedRoot>, SourceDiscoveryError> {
    let mut roots = Vec::new();
    append_roots(manifest, Some(target), SourceRole::Production, &mut roots)?;
    if include_tests {
        append_roots(manifest, Some(target), SourceRole::Test, &mut roots)?;
    }
    Ok(roots)
}

pub(super) fn formatting_roots(
    manifest: &LoadedConeManifest,
) -> Result<Vec<SelectedRoot>, SourceDiscoveryError> {
    let mut roots = Vec::new();
    append_roots(manifest, None, SourceRole::Production, &mut roots)?;
    append_roots(manifest, None, SourceRole::Test, &mut roots)?;
    Ok(roots)
}

fn append_roots(
    manifest: &LoadedConeManifest,
    target: Option<TargetProfileId>,
    role: SourceRole,
    roots: &mut Vec<SelectedRoot>,
) -> Result<(), SourceDiscoveryError> {
    let (selection, default_path, field) = match role {
        SourceRole::Production => (manifest.parsed().semantic().sources(), "src", "sources"),
        SourceRole::Test => (manifest.parsed().tests().sources(), "tests", "test.sources"),
    };
    let paths: Vec<_> = match selection {
        SourceSelection::Default => vec![(
            ConeRelativePath::new(default_path).expect("the default source path is relative"),
            format!("default {default_path}/"),
        )],
        SourceSelection::Explicit(paths) => paths
            .iter()
            .enumerate()
            .filter(|(_, path)| target.is_none_or(|target| path.predicate().matches(target)))
            .map(|(index, path)| {
                (
                    path.path().clone(),
                    format!("{field}[{}] ({:?})", index + 1, path.path().as_str()),
                )
            })
            .collect(),
    };
    for (path, label) in paths {
        let display_path = manifest.real_root().join(path.as_path());
        if role == SourceRole::Test
            && matches!(selection, SourceSelection::Default)
            && matches!(std::fs::symlink_metadata(&display_path), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
        {
            continue;
        }
        let resolved = resolve_entry(&display_path)?;
        if matches!(selection, SourceSelection::Default) && !resolved.metadata.is_dir() {
            return Err(SourceDiscoveryError::new(
                display_path,
                match role {
                    SourceRole::Production => SourceDiscoveryErrorKind::SourceRootNotDirectory,
                    SourceRole::Test => SourceDiscoveryErrorKind::TestRootNotDirectory,
                },
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
            role,
        };
        for prior in roots.iter().filter(|_| target.is_some()) {
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
    Ok(())
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
