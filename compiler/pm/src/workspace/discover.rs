use super::{load::Manifests, *};

pub(super) fn discover(cwd: &Path, input: Option<&Path>) -> PmResult<Workspace> {
    let mut manifests = Manifests::default();
    let selected = manifests.load(&input_manifest(cwd, input)?)?;
    let owner = find_owner(&selected, &mut manifests)?;
    let definition = owner.document.workspace.as_ref();
    let mut paths = Vec::new();
    if owner.document.cone.is_some() {
        paths.push((".".to_owned(), owner.clone()));
    }
    if let Some(definition) = definition {
        for member in &definition.members {
            let path = owner
                .directory
                .join(member.value.as_path())
                .join("Cone.toml");
            let loaded = manifests.load(&path)?;
            if loaded.document.workspace.is_some() {
                return Err(
                    PmError::new("a nested workspace cannot be an outer workspace member")
                        .at(owner.origin(member.span.clone())),
                );
            }
            if paths
                .iter()
                .any(|(_, existing)| existing.path == loaded.path)
            {
                return Err(PmError::new("duplicate workspace member")
                    .at(owner.origin(member.span.clone())));
            }
            paths.push((member.value.as_str().to_owned(), loaded));
        }
    }
    let mut members: BTreeMap<DependencyCoordinateKey, WorkspaceMember> = BTreeMap::new();
    let mut invoking_member = None;
    for (relative_path, loaded) in paths {
        let cone = expand::expand(&loaded, definition.map(|_| owner.as_ref()))?;
        let key = DependencyCoordinateKey::from_coordinate(&cone.coordinate);
        if let Some(previous) = members.get(&key) {
            return Err(PmError::new(format!(
                "duplicate workspace coordinate {key}: {} and {}",
                previous.manifest.display(),
                loaded.path.display()
            )));
        }
        if selected.path != owner.path && selected.path == loaded.path {
            invoking_member = Some(key.clone());
        }
        members.insert(
            key,
            WorkspaceMember {
                relative_path,
                directory: loaded.directory.clone(),
                manifest: loaded.path.clone(),
                cone,
            },
        );
    }
    let defaults = if let Some(paths) =
        definition.and_then(|definition| definition.default_members.as_ref())
    {
        let mut defaults = BTreeSet::new();
        for path in paths {
            let key = members
                .iter()
                .find(|(_, member)| member.relative_path == path.value.as_str())
                .map(|(key, _)| key.clone())
                .ok_or_else(|| {
                    PmError::new(format!(
                        "default member {:?} is not a workspace member",
                        path.value.as_str()
                    ))
                    .at(owner.origin(path.span.clone()))
                })?;
            if !defaults.insert(key) {
                return Err(PmError::new("duplicate default workspace member")
                    .at(owner.origin(path.span.clone())));
            }
        }
        defaults.into_iter().collect()
    } else if owner.document.cone.is_some() {
        members
            .iter()
            .filter(|(_, member)| member.relative_path == ".")
            .map(|(key, _)| key.clone())
            .collect()
    } else {
        members.keys().cloned().collect()
    };
    Ok(Workspace {
        root: owner.directory.clone(),
        manifest: owner.path.clone(),
        document: owner.document.clone(),
        members,
        defaults,
        invoking_member,
    })
}

fn find_owner(
    selected: &Arc<LoadedManifest>,
    manifests: &mut Manifests,
) -> PmResult<Arc<LoadedManifest>> {
    if selected.document.workspace.is_some() {
        return Ok(selected.clone());
    }
    for directory in selected.directory.ancestors().skip(1) {
        let path = directory.join("Cone.toml");
        if !exists(&path)? {
            continue;
        }
        let parent = manifests.load(&path)?;
        if let Some(workspace) = &parent.document.workspace {
            for member in &workspace.members {
                let path = parent
                    .directory
                    .join(member.value.as_path())
                    .join("Cone.toml");
                // An unrelated, unlisted Cone does not validate other workspace members.
                let member_path = match std::fs::canonicalize(&path) {
                    Ok(path) => path,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(PmError::io(&path, error)),
                };
                if member_path == selected.path {
                    return Ok(parent.clone());
                }
            }
            break;
        }
    }
    Ok(selected.clone())
}

fn input_manifest(cwd: &Path, input: Option<&Path>) -> PmResult<PathBuf> {
    if let Some(input) = input {
        let path = if input.is_absolute() {
            input.to_path_buf()
        } else {
            cwd.join(input)
        };
        let metadata = std::fs::metadata(&path).map_err(|error| PmError::io(&path, error))?;
        return if metadata.is_dir() {
            Ok(path.join("Cone.toml"))
        } else if metadata.is_file() && path.file_name().is_some_and(|name| name == "Cone.toml") {
            Ok(path)
        } else {
            Err(PmError::new("input must be a Cone directory or Cone.toml"))
        };
    }
    for directory in cwd.ancestors() {
        let path = directory.join("Cone.toml");
        if exists(&path)? {
            return Ok(path);
        }
    }
    Err(PmError::new(format!(
        "no Cone.toml found from {} or its parents",
        cwd.display()
    )))
}

fn exists(path: &Path) -> PmResult<bool> {
    match std::fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(PmError::io(path, error)),
    }
}
