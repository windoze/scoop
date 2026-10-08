use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use scoop_lir::{OptimizationMode, TargetProfileId, ValidatedCBridgeToolchainInvocation};
use scoop_manifest::{ImmutableInputSnapshot, LoadedConeManifest, NativeCompileFlag};
use scoop_process::parse_make_dependencies;

use super::paths::{InputRoots, normalize_line_markers};
use super::{NativeSourceInput, PreparedNativeInputs, ToolchainError, command, error};

pub(super) fn prepare(
    manifest: &LoadedConeManifest,
    target: TargetProfileId,
    compiler: &ValidatedCBridgeToolchainInvocation,
    optimization: OptimizationMode,
    public_include: &Path,
) -> Result<PreparedNativeInputs, ToolchainError> {
    let config = manifest.parsed().semantic().native();
    let mut sources: Vec<_> = config
        .sources()
        .iter()
        .filter(|source| source.predicate().matches(target))
        .collect();
    sources.sort_by_key(|source| source.path());
    if sources.is_empty() {
        return Ok(PreparedNativeInputs::default());
    }
    let roots = InputRoots {
        cone: manifest.real_root().to_path_buf(),
        public: public_include.canonicalize().map_err(error)?,
        system: command::system_roots(compiler)?,
    };
    roots.validate_includes(config)?;
    let mut selected = BTreeSet::new();
    for source in &sources {
        let resolved = roots.checked_path(source.path(), false)?;
        if !selected.insert(super::paths::physical_identity(&resolved)?) {
            return Err(error(format!(
                "duplicate native source {}",
                source.path().as_str()
            )));
        }
        match source
            .path()
            .as_path()
            .extension()
            .and_then(|ext| ext.to_str())
        {
            Some("c") => {}
            Some("cc" | "cpp" | "cxx" | "C") => {
                return Err(error(format!(
                    "{} requires native.cxx = true",
                    source.path().as_str()
                )));
            }
            _ => {
                return Err(error(format!(
                    "{} has an invalid native source extension",
                    source.path().as_str()
                )));
            }
        }
    }
    let directory = tempfile::Builder::new()
        .prefix("scoop-preprocess-")
        .tempdir()
        .map_err(error)?;
    for _ in 0..3 {
        let mut result = PreparedNativeInputs::default();
        let mut captured = BTreeMap::new();
        for source in &sources {
            let input = roots.cone.join(source.path().as_path());
            let depfile = directory.path().join("unit.d");
            let mut command = command::base(compiler, optimization);
            command.args([
                format!("-DSCOOP_TARGET_OS_{}=1", target.os().to_ascii_uppercase()),
                format!(
                    "-DSCOOP_TARGET_ARCH_{}=1",
                    target.arch().to_ascii_uppercase()
                ),
                format!("-DSCOOP_TARGET_ENV_{}=1", target.env().to_ascii_uppercase()),
            ]);
            command::configure_preprocessor(&mut command, config, &roots.cone);
            command
                .current_dir(&roots.cone)
                .arg("-I")
                .arg(&roots.public);
            for (root, logical) in roots.mappings() {
                command.arg(format!("-ffile-prefix-map={}={logical}", root.display()));
            }
            command
                .arg("-fPIC")
                .args(["-E", "-x", "c", "-MD", "-MT", "native-unit", "-MF"])
                .arg(&depfile)
                .arg(&input);
            let output = command::run(&mut command, source.path().as_str())?;
            let dependencies = std::fs::read_to_string(&depfile).map_err(error)?;
            let dependencies = parse_make_dependencies(&dependencies, "native-unit")
                .ok_or_else(|| error("C compiler returned an invalid dependency file"))?;
            let mut path_map = BTreeMap::new();
            for path in dependencies {
                let path = if path.is_absolute() {
                    path
                } else {
                    roots.cone.join(path)
                };
                let logical = roots.logical(&path)?;
                let snapshot = if let Some(snapshot) = captured.get(&path) {
                    snapshot
                } else {
                    captured
                        .entry(path.clone())
                        .or_insert(ImmutableInputSnapshot::capture(&path).map_err(error)?)
                };
                result
                    .dependencies
                    .insert(logical.clone(), snapshot.digest());
                path_map.insert(snapshot.resolved_path().to_path_buf(), logical.clone());
                path_map.insert(path, logical);
            }
            let preprocessed = normalize_line_markers(&output.stdout, &path_map, &roots)?;
            result.units.push(NativeSourceInput::new(
                source.path().clone(),
                preprocessed.into(),
            ));
        }
        if captured.values().all(ImmutableInputSnapshot::is_current) {
            result.flags = config
                .include()
                .iter()
                .flat_map(|path| ["-I".to_owned(), path.as_str().to_owned()])
                .chain(config.c_flags().iter().flat_map(|flag| match flag {
                    NativeCompileFlag::Argument(argument) => vec![argument.clone()],
                    NativeCompileFlag::Include { kind, path } => {
                        vec![kind.argument().to_owned(), path.as_str().to_owned()]
                    }
                }))
                .collect();
            return Ok(result);
        }
    }
    Err(error(format!(
        "inputs for {} kept changing during preprocessing",
        Path::new(manifest.manifest_path()).display()
    )))
}
