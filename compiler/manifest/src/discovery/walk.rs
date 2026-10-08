use super::*;

pub(super) fn walk_directory(
    real_directory: &Path,
    logical_directory: &Path,
    cone_root: &Path,
    selection: &str,
    active_directories: &mut BTreeSet<PathBuf>,
    candidates: &mut Vec<SourceCandidate>,
) -> Result<(), SourceDiscoveryError> {
    if !active_directories.insert(real_directory.to_path_buf()) {
        return Err(SourceDiscoveryError::new(
            real_directory.to_path_buf(),
            SourceDiscoveryErrorKind::DirectoryCycle,
        ));
    }

    let result = walk_active_directory(
        real_directory,
        logical_directory,
        cone_root,
        selection,
        active_directories,
        candidates,
    );
    active_directories.remove(real_directory);
    result
}

fn walk_active_directory(
    real_directory: &Path,
    logical_directory: &Path,
    cone_root: &Path,
    selection: &str,
    active_directories: &mut BTreeSet<PathBuf>,
    candidates: &mut Vec<SourceCandidate>,
) -> Result<(), SourceDiscoveryError> {
    let entries = std::fs::read_dir(real_directory).map_err(|error| {
        SourceDiscoveryError::io(
            DiscoveryIoOperation::ListDirectory,
            real_directory.to_path_buf(),
            error,
        )
    })?;
    let mut entries = entries
        .map(|entry| {
            entry.map_err(|error| {
                SourceDiscoveryError::io(
                    DiscoveryIoOperation::ListDirectory,
                    real_directory.to_path_buf(),
                    error,
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for entry in &entries {
        if validated_entry_name(&entry.file_name()).is_none() {
            return Err(SourceDiscoveryError::new(
                entry.path(),
                SourceDiscoveryErrorKind::NonUtf8EntryName,
            ));
        }
    }
    entries.sort_by(|left, right| {
        left.file_name()
            .to_str()
            .expect("entry names were validated as UTF-8")
            .as_bytes()
            .cmp(
                right
                    .file_name()
                    .to_str()
                    .expect("entry names were validated as UTF-8")
                    .as_bytes(),
            )
    });

    for entry in entries {
        let name = entry.file_name();
        let name = name.to_str().expect("entry names were validated as UTF-8");
        let logical_path = logical_directory.join(name);
        let resolved = resolve_entry(&entry.path())?;
        if !resolved.path.starts_with(cone_root) {
            return Err(SourceDiscoveryError::new(
                entry.path(),
                SourceDiscoveryErrorKind::EscapesConeRoot,
            ));
        }

        if resolved.metadata.is_dir() {
            walk_directory(
                &resolved.path,
                &logical_path,
                cone_root,
                selection,
                active_directories,
                candidates,
            )?;
        } else if resolved.metadata.is_file()
            && logical_path.extension() == Some(OsStr::new("scoop"))
        {
            let normalized =
                NormalizedSourcePath::from_relative_path(&logical_path).map_err(|error| {
                    SourceDiscoveryError::new(
                        entry.path(),
                        SourceDiscoveryErrorKind::InvalidLogicalPath(error),
                    )
                })?;

            candidates.try_reserve(1).map_err(|_| {
                SourceDiscoveryError::new(
                    entry.path(),
                    SourceDiscoveryErrorKind::Allocation {
                        requested_bytes: u64::try_from(std::mem::size_of::<SourceCandidate>())
                            .unwrap_or(u64::MAX),
                    },
                )
            })?;
            candidates.push(SourceCandidate {
                logical_path: normalized,
                physical_path: resolved.path,
                display_path: entry.path(),
                selection: selection.to_owned(),
            });
        }
    }
    Ok(())
}

pub(super) fn validated_entry_name(name: &OsStr) -> Option<&str> {
    name.to_str()
}

pub(super) fn resolve_entry(path: &Path) -> Result<ResolvedEntry, SourceDiscoveryError> {
    let mut current = path.to_path_buf();
    let mut links = BTreeSet::new();
    loop {
        let metadata = std::fs::symlink_metadata(&current).map_err(|error| {
            SourceDiscoveryError::io(DiscoveryIoOperation::Inspect, current.clone(), error)
        })?;
        if !metadata.file_type().is_symlink() {
            let canonical = std::fs::canonicalize(&current).map_err(|error| {
                SourceDiscoveryError::io(DiscoveryIoOperation::Canonicalize, current.clone(), error)
            })?;
            let metadata = std::fs::metadata(&canonical).map_err(|error| {
                SourceDiscoveryError::io(DiscoveryIoOperation::Inspect, canonical.clone(), error)
            })?;
            return Ok(ResolvedEntry {
                path: canonical,
                metadata,
            });
        }
        let parent = current
            .parent()
            .expect("a symlink directory entry has a parent");
        let parent = std::fs::canonicalize(parent).map_err(|error| {
            SourceDiscoveryError::io(DiscoveryIoOperation::Canonicalize, current.clone(), error)
        })?;
        let link = parent.join(
            current
                .file_name()
                .expect("a symlink directory entry has a name"),
        );
        if !links.insert(link) {
            return Err(SourceDiscoveryError::new(
                path.to_path_buf(),
                SourceDiscoveryErrorKind::SymlinkCycle,
            ));
        }
        let target = std::fs::read_link(&current).map_err(|error| {
            SourceDiscoveryError::io(DiscoveryIoOperation::ReadLink, current.clone(), error)
        })?;
        current = if target.is_absolute() {
            target
        } else {
            parent.join(target)
        };
    }
}
