use super::*;
use crate::native_input::{NativeContent, NativeFile, NativeFileKind};

pub(super) fn dependency(
    parent: &DynamicProvider,
    name: &str,
    roots: &[PathBuf],
    profile: &ValidatedFinalLinkProfile,
) -> Result<NativeFile, LinkError> {
    let mut paths = BTreeMap::new();
    let mut add = |path: PathBuf, system: bool| {
        let text = path.extension().is_some_and(|extension| extension == "tbd");
        paths.insert(
            path.clone(),
            (
                if text {
                    NativeFileKind::TextStub
                } else {
                    NativeFileKind::Dylib
                },
                system,
            ),
        );
        if path
            .extension()
            .is_some_and(|extension| extension == "dylib")
        {
            paths.insert(
                path.with_extension("tbd"),
                (NativeFileKind::TextStub, system),
            );
        }
    };
    if let Some(suffix) = name.strip_prefix("@loader_path/") {
        add(
            parent
                .locator
                .parent()
                .ok_or_else(|| error("dynamic provider has no directory"))?
                .join(suffix),
            false,
        );
    } else if let Some(suffix) = name.strip_prefix("@rpath/") {
        for root in roots {
            add(root.join(suffix), false);
        }
        for root in &parent.rpaths {
            add(loader_directory(root, parent, roots)?.join(suffix), false);
        }
    } else if name.starts_with('/') {
        let path = Path::new(name);
        for root in roots {
            add(root.join(name.trim_start_matches('/')), false);
            if let Some(basename) = path.file_name() {
                add(root.join(basename), false);
            }
            if let Some((prefix, rest)) = name.split_once(".framework/") {
                if let Some(framework) = Path::new(prefix).file_name() {
                    add(
                        root.join(format!("{}.framework/{rest}", framework.to_string_lossy())),
                        false,
                    );
                }
            }
        }
        if name.starts_with("/usr/lib/") || name.starts_with("/System/Library/") {
            let path = profile
                .startup_toolchain()
                .sdk_root()
                .map_err(error)?
                .join(name.trim_start_matches('/'));
            add(path.with_extension("tbd"), true);
        }
    } else {
        return Err(error(format!(
            "unsupported native load path {name:?} from {}",
            parent.install_name
        )));
    }
    let mut candidates = BTreeMap::new();
    let mut found = Vec::new();
    for (path, (kind, system)) in paths {
        match std::fs::symlink_metadata(&path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(error(format!(
                    "dynamic dependency {}: {err}",
                    path.display()
                )));
            }
            Ok(_) => {}
        }
        let file =
            crate::native_input::locate::read(&path, kind, profile, system).map_err(|err| {
                error(format!(
                    "dynamic dependency {} from {}: {err}",
                    path.display(),
                    parent.install_name
                ))
            })?;
        if let NativeContent::Dynamic(records) = &file.content {
            if !name.starts_with("@loader_path/")
                && !records.iter().any(|record| record.install_name == name)
            {
                return Err(error(format!(
                    "dynamic dependency {} does not declare install name {name}",
                    path.display()
                )));
            }
        }
        found.push(path);
        if candidates
            .get(&file.id)
            .is_none_or(|previous: &NativeFile| file.locator < previous.locator)
        {
            candidates.insert(file.id, file);
        }
    }
    if candidates.len() != 1 {
        return Err(error(format!(
            "dynamic dependency {name} from {} has {} candidates: {found:?}",
            parent.install_name,
            candidates.len()
        )));
    }
    candidates
        .into_values()
        .next()
        .ok_or_else(|| error("dynamic dependency candidate disappeared"))
}

fn loader_directory(
    value: &str,
    parent: &DynamicProvider,
    roots: &[PathBuf],
) -> Result<PathBuf, LinkError> {
    if value == "@loader_path" {
        return parent
            .locator
            .parent()
            .map(Path::to_owned)
            .ok_or_else(|| error("dynamic provider has no parent"));
    }
    if let Some(suffix) = value.strip_prefix("@loader_path/") {
        return Ok(parent
            .locator
            .parent()
            .ok_or_else(|| error("dynamic provider has no parent"))?
            .join(suffix));
    }
    if value.starts_with('/')
        && roots
            .iter()
            .any(|root| std::fs::canonicalize(root).ok().as_deref() == Some(Path::new(value)))
    {
        return Ok(PathBuf::from(value));
    }
    Err(error(format!(
        "native provider {} has unexplained RPATH {value:?}",
        parent.install_name
    )))
}

pub(super) fn runpath(
    provider: &DynamicProvider,
    roots: &[PathBuf],
) -> Result<Option<String>, LinkError> {
    if provider.install_name.starts_with('/') {
        return Ok(None);
    }
    let suffix = provider
        .install_name
        .strip_prefix("@rpath/")
        .ok_or_else(|| {
            error(format!(
                "direct dynamic provider has unsupported install name {}",
                provider.install_name
            ))
        })?;
    let mut matching = BTreeSet::new();
    for root in roots {
        if std::fs::canonicalize(root.join(suffix)).ok().as_ref() == Some(&provider.locator)
            || std::fs::canonicalize(root.join(suffix).with_extension("tbd"))
                .ok()
                .as_ref()
                == Some(&provider.locator)
        {
            matching.insert(
                std::fs::canonicalize(root)
                    .map_err(error)?
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    matching.pop_first().map(Some).ok_or_else(|| {
        error(format!(
            "install name {} cannot be located through an explicit library root",
            provider.install_name
        ))
    })
}
