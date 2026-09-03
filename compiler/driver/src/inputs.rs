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
        name: user_path.display().to_string(),
        source,
    });
    Ok(inputs)
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
