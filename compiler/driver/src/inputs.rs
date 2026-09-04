use super::*;

/// Load the compilation unit: every `src/*.scoop` of the sysroot's
/// `scoop.core` Cone (sorted by file name) followed by the user file.
pub fn load_inputs(user_path: &Path) -> Result<Vec<SourceFileInput>, Vec<Diagnostic>> {
    let mut inputs = load_core_sources()?;
    let user_index = inputs.len();
    let source = std::fs::read_to_string(user_path).map_err(|e| {
        vec![no_span(
            user_index,
            format!("cannot read {}: {e}", user_path.display()),
        )]
    })?;
    inputs.push(SourceFileInput {
        name: stable_user_source_name(user_path),
        source,
    });
    Ok(inputs)
}

/// Produce a host-independent source identity for HIR declaration keys. M21
/// accepts one user file, so a containing Cone (when present) is the strongest
/// root; otherwise the current compilation directory is the portable root.
fn stable_user_source_name(user_path: &Path) -> String {
    if !user_path.is_absolute() {
        return normalized_relative_path(user_path);
    }
    let canonical = user_path
        .canonicalize()
        .unwrap_or_else(|_| user_path.to_path_buf());
    let cone_root = canonical
        .ancestors()
        .find(|ancestor| ancestor.join("Cone.toml").is_file());
    let relative = cone_root
        .and_then(|root| canonical.strip_prefix(root).ok())
        .map(Path::to_path_buf)
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|dir| dir.canonicalize().ok())
                .and_then(|dir| canonical.strip_prefix(dir).ok().map(Path::to_path_buf))
        })
        .or_else(|| canonical.file_name().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("<user>"));
    normalized_relative_path(&relative)
}

fn normalized_relative_path(path: &Path) -> String {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(value) => {
                components.push(value.to_string_lossy().into_owned());
            }
            std::path::Component::ParentDir => {
                if components.pop().is_none() {
                    components.push("__parent__".to_string());
                }
            }
            std::path::Component::CurDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {}
        }
    }
    if components.is_empty() {
        "<user>".to_string()
    } else {
        components.join("/")
    }
}

/// Render diagnostics against the loaded inputs, selecting each
/// diagnostic's `(name, source)` by its file index. Driver-level
/// diagnostics emitted before loading finished have no corresponding
/// input; they fall back to the user file's name and source.
pub fn render_diagnostics(
    diagnostics: &[Diagnostic],
    inputs: &[SourceFileInput],
    fallback_name: &str,
    fallback_source: &str,
) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| match inputs.get(diagnostic.file) {
            Some(input) => diagnostic.render(&input.name, &input.source),
            None => diagnostic.render(fallback_name, fallback_source),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Locate the sysroot: `SCOOP_SYSROOT` if set, otherwise the workspace's
/// `sysroot/` directory.
fn sysroot() -> PathBuf {
    match std::env::var_os("SCOOP_SYSROOT") {
        Some(dir) => PathBuf::from(dir),
        None => workspace_root().join("sysroot"),
    }
}

/// Read the `scoop.core` Cone of the located sysroot: validate its
/// `Cone.toml`, then load every `src/*.scoop` sorted by file name.
fn load_core_sources() -> Result<Vec<SourceFileInput>, Vec<Diagnostic>> {
    let core_dir = sysroot().join("lib/scoop.core");
    let manifest_path = core_dir.join("Cone.toml");
    let manifest = std::fs::read_to_string(&manifest_path).map_err(|e| {
        vec![no_span(
            0,
            format!("cannot read {}: {e}", manifest_path.display()),
        )]
    })?;
    validate_core_manifest(&manifest, &manifest_path)?;

    let src_dir = core_dir.join("src");
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&src_dir).map_err(|e| {
        vec![no_span(
            0,
            format!("cannot list {}: {e}", src_dir.display()),
        )]
    })? {
        let path = entry
            .map_err(|e| {
                vec![no_span(
                    0,
                    format!("cannot read {}: {e}", src_dir.display()),
                )]
            })?
            .path();
        if path.extension().is_some_and(|ext| ext == "scoop") {
            entries.push(path);
        }
    }
    // All entries share `src_dir`, so path order is file-name order.
    entries.sort();

    let mut inputs = Vec::with_capacity(entries.len());
    for (index, path) in entries.iter().enumerate() {
        let source = std::fs::read_to_string(path).map_err(|e| {
            vec![no_span(
                index,
                format!("cannot read {}: {e}", path.display()),
            )]
        })?;
        let file_name = path
            .file_name()
            .expect("dir entry has a file name")
            .to_string_lossy();
        inputs.push(SourceFileInput {
            name: format!("scoop.core/src/{file_name}"),
            source,
        });
    }
    Ok(inputs)
}

/// Validate the M4 slice of a `Cone.toml`: a `[cone]` table with string
/// `group` / `name` / `version`, and `name` matching the Cone directory.
fn validate_core_manifest(manifest: &str, path: &Path) -> Result<(), Vec<Diagnostic>> {
    let invalid =
        |detail: String| vec![no_span(0, format!("invalid {}: {detail}", path.display()))];
    let table = manifest
        .parse::<toml::Table>()
        .map_err(|e| invalid(format!("not valid TOML: {e}")))?;
    let Some(cone) = table.get("cone").and_then(toml::Value::as_table) else {
        return Err(invalid("missing [cone] section".into()));
    };
    for field in ["group", "name", "version"] {
        if cone.get(field).and_then(toml::Value::as_str).is_none() {
            return Err(invalid(format!("missing [cone].{field}")));
        }
    }
    let name = cone["name"].as_str().expect("checked above");
    if name != "scoop.core" {
        return Err(invalid(format!(
            "[cone].name is {name:?}, expected \"scoop.core\""
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_source_identity_is_independent_from_absolute_invocation() {
        let relative = Path::new("tests/fixtures/m21-initialization/top-level-runtime.scoop");
        let absolute = std::env::current_dir()
            .expect("current directory")
            .join(relative);
        assert_eq!(
            stable_user_source_name(relative),
            relative.to_string_lossy()
        );
        assert_eq!(
            stable_user_source_name(&absolute),
            relative.to_string_lossy()
        );
    }
}
