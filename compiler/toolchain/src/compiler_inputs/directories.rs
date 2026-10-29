//! Enumerate and validate all entries before reading the selected regular files.

use super::{
    files::Sources,
    paths::{Scope, carrier, io_error, modified},
    patterns::Patterns,
};
use scoop_hir::{
    CompileTimeDirectoryError as Error, CompileTimeDirectoryFilter, CompileTimeFile,
    CompileTimePath,
};
use scoop_identity::SourceIdentity;
use scoop_protocol::{DirectoryEntryInputV1, DirectoryEntryStateV1, DirectoryInputV1};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone)]
pub(super) struct ReadDirectory {
    pub files: Arc<[CompileTimeFile]>,
    pub input: DirectoryInputV1,
}

pub(super) fn read(
    sources: &Sources,
    source: &SourceIdentity,
    path: &CompileTimePath,
    filter: &CompileTimeDirectoryFilter,
) -> Result<ReadDirectory, Error> {
    let patterns = Patterns::new(filter)?;
    let locator = sources.locator(source, path).map_err(Error::Input)?;
    let input = inspect(&sources.root, &locator, filter, &patterns).map_err(Error::Input)?;
    let mut content = BTreeMap::new();
    let mut files = Vec::new();
    for entry in &input.entries {
        if !matches!(entry.state, DirectoryEntryStateV1::File { .. }) {
            continue;
        }
        let bytes = match content.get(&entry.resolved) {
            Some(bytes) => Arc::clone(bytes),
            None => {
                let resolved = entry
                    .resolved
                    .to_path_buf()
                    .map_err(|error| Error::Input(error.to_string()))?;
                let bytes: Arc<[u8]> = fs::read(&resolved)
                    .map_err(|error| Error::Input(io_error(&locator.join(&entry.path), error)))?
                    .into();
                content.insert(entry.resolved.clone(), bytes.clone());
                bytes
            }
        };
        files.push(CompileTimeFile {
            path: entry.path.clone(),
            bytes,
        });
    }
    Ok(ReadDirectory {
        files: files.into(),
        input,
    })
}

pub(super) fn current(input: &DirectoryInputV1) -> bool {
    let (Ok(root), Ok(locator)) = (input.root.to_path_buf(), input.locator.to_path_buf()) else {
        return false;
    };
    let filter = CompileTimeDirectoryFilter {
        include: input.include.clone(),
        exclude: input.exclude.clone(),
    };
    let Ok(patterns) = Patterns::new(&filter) else {
        return false;
    };
    inspect(&root, &locator, &filter, &patterns).is_ok_and(|observed| observed == *input)
}

fn inspect(
    root: &Path,
    locator: &Path,
    filter: &CompileTimeDirectoryFilter,
    patterns: &Patterns,
) -> Result<DirectoryInputV1, String> {
    let scope = Scope::new(root)?;
    let resolved = scope.resolve(locator)?;
    if !fs::metadata(&resolved)
        .map_err(|error| io_error(locator, error))?
        .is_dir()
    {
        return Err(format!(
            "includeDir path `{}` must name a directory",
            locator.display()
        ));
    }
    let mut walk = Walk {
        scope: &scope,
        patterns,
        ancestors: BTreeSet::new(),
        entries: Vec::new(),
    };
    walk.directory(locator, &resolved, "")?;
    walk.entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(DirectoryInputV1 {
        root: carrier(root)?,
        real_root: carrier(&scope.real_root)?,
        locator: carrier(locator)?,
        resolved: carrier(&resolved)?,
        include: filter.include.clone(),
        exclude: filter.exclude.clone(),
        entries: walk.entries,
    })
}

struct Walk<'a> {
    scope: &'a Scope,
    patterns: &'a Patterns,
    ancestors: BTreeSet<PathBuf>,
    entries: Vec<DirectoryEntryInputV1>,
}

impl Walk<'_> {
    fn directory(&mut self, locator: &Path, resolved: &Path, prefix: &str) -> Result<(), String> {
        if !self.ancestors.insert(resolved.to_owned()) {
            return Err(format!(
                "includeDir directory cycle at `{}`",
                locator.display()
            ));
        }
        let mut entries = fs::read_dir(resolved)
            .map_err(|error| io_error(locator, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| io_error(locator, error))?;
        entries.sort_by_cached_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry.file_name();
            let path = locator.join(&name);
            let name = name.to_str().ok_or_else(|| {
                format!(
                    "includeDir entry `{}` must have a valid UTF-8 path",
                    path.display()
                )
            })?;
            let relative = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}/{name}")
            };
            self.entry(&path, &relative)
                .map_err(|message| format!("includeDir entry `{relative}`: {message}"))?;
        }
        self.ancestors.remove(resolved);
        Ok(())
    }

    fn entry(&mut self, locator: &Path, relative: &str) -> Result<(), String> {
        let resolved = self.scope.resolve(locator)?;
        let metadata = fs::metadata(&resolved).map_err(|error| io_error(locator, error))?;
        let state = if metadata.is_dir() {
            self.directory(locator, &resolved, relative)?;
            DirectoryEntryStateV1::Directory
        } else if self.patterns.matches(relative) {
            if !metadata.is_file() {
                return Err(format!(
                    "includeDir path `{}` must name a regular file",
                    locator.display()
                ));
            }
            DirectoryEntryStateV1::File {
                length: metadata.len(),
                modified: modified(locator, &metadata)?,
            }
        } else {
            DirectoryEntryStateV1::Unselected
        };
        self.entries.push(DirectoryEntryInputV1 {
            path: relative.to_owned(),
            resolved: carrier(&resolved)?,
            state,
        });
        Ok(())
    }
}
