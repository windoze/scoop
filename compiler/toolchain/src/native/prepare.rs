use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use scoop_lir::{OptimizationMode, TargetProfileId};
use scoop_manifest::{LoadedConeManifest, NativeCompileFlag, NativeConfig, NativeSourceLanguage};
use scoop_process::parse_make_dependencies;

use super::paths::{InputRoots, normalize_line_markers};
use super::{
    NativeSourceInput, NativeToolchain, PreparedNativeInputs, ToolchainError, command, error,
};

pub(super) fn prepare(
    manifest: &LoadedConeManifest,
    target: TargetProfileId,
    compiler: &NativeToolchain,
    optimization: OptimizationMode,
    public_include: &Path,
) -> Result<PreparedNativeInputs, ToolchainError> {
    let config = manifest.parsed().semantic().native();
    let libraries = config.library_requirements(target).map_err(error)?;
    let cxx = compiler.cxx_fingerprint();
    let mut sources: Vec<_> = config
        .sources()
        .iter()
        .filter(|source| source.predicate().matches(target))
        .collect();
    sources.sort_by_key(|source| source.path());
    if let Some(pair) = sources
        .windows(2)
        .find(|pair| pair[0].path() == pair[1].path())
    {
        return Err(error(format!(
            "duplicate native source {}",
            pair[0].path().as_str()
        )));
    }
    if sources.is_empty() {
        return Ok(PreparedNativeInputs {
            libraries,
            cxx,
            ..PreparedNativeInputs::default()
        });
    }
    let sources = sources
        .into_iter()
        .map(|source| {
            config
                .source_language(source.path())
                .map(|language| (source, language))
                .map_err(error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let languages: BTreeSet<_> = sources.iter().map(|(_, language)| *language).collect();
    let mut system = Vec::new();
    for language in &languages {
        for root in command::system_roots(compiler, *language)? {
            if !system.contains(&root) {
                system.push(root);
            }
        }
    }
    let roots = InputRoots {
        cone: manifest.real_root().to_path_buf(),
        public: public_include.canonicalize().map_err(error)?,
        system,
    };
    for (source, _) in &sources {
        roots.check_source(source.path().as_path())?;
    }
    let directory = tempfile::Builder::new()
        .prefix("scoop-preprocess-")
        .tempdir()
        .map_err(error)?;
    let target_facts = crate::manifest::source_target(target)?;
    let mut result = PreparedNativeInputs {
        libraries: libraries.clone(),
        cxx,
        ..PreparedNativeInputs::default()
    };
    let mut search = vec![roots.cone.clone(), roots.public.clone()];
    search.extend(roots.system.clone());
    search.extend(
        config
            .include()
            .iter()
            .map(|p| roots.cone.join(p.as_path())),
    );
    for language in &languages {
        for flag in config.flags(*language) {
            if let NativeCompileFlag::Include { kind, path } = flag {
                let path = roots.cone.join(path.as_path());
                if matches!(
                    kind,
                    scoop_manifest::NativeIncludeFlag::ForcedHeader
                        | scoop_manifest::NativeIncludeFlag::Macros
                ) {
                    result.observed_paths.insert(path);
                } else {
                    search.push(path);
                }
            }
        }
    }
    for (source, language) in &sources {
        let input = roots.cone.join(source.path().as_path());
        let depfile = directory.path().join("unit.d");
        let mut command = command::base(compiler, *language, optimization)?;
        command.args([
            format!(
                "-DSCOOP_TARGET_OS_{}=1",
                target_facts.os.to_ascii_uppercase()
            ),
            format!(
                "-DSCOOP_TARGET_ARCH_{}=1",
                target_facts.arch.to_ascii_uppercase()
            ),
            format!(
                "-DSCOOP_TARGET_ENV_{}=1",
                target_facts.env.to_ascii_uppercase()
            ),
        ]);
        command::configure_preprocessor(&mut command, config, *language, &roots.cone);
        command
            .current_dir(&roots.cone)
            .arg("-I")
            .arg(&roots.public);
        for (root, logical) in roots.mappings() {
            command.arg(format!("-ffile-prefix-map={}={logical}", root.display()));
        }
        command
            .arg("-fPIC")
            .args([
                "-E",
                "-dI",
                "-x",
                language.input_name(),
                "-MD",
                "-MT",
                "native-unit",
                "-MF",
            ])
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
            result.observed_paths.insert(path.clone());
            path_map.insert(path.canonicalize().map_err(error)?, logical.clone());
            path_map.insert(path, logical);
        }
        let preprocessed = normalize_line_markers(
            &output.stdout,
            &path_map,
            &roots,
            &search,
            &mut result.observed_paths,
        )?;
        result.units.push(NativeSourceInput::new(
            source.path().clone(),
            *language,
            preprocessed.into(),
        ));
    }
    if languages.contains(&NativeSourceLanguage::C) {
        result.flags = flags(config, NativeSourceLanguage::C);
    }
    if languages.contains(&NativeSourceLanguage::Cxx) {
        result.cxx_flags = flags(config, NativeSourceLanguage::Cxx);
    }
    Ok(result)
}

fn flags(config: &NativeConfig, language: NativeSourceLanguage) -> Vec<String> {
    config
        .include()
        .iter()
        .flat_map(|path| ["-I".to_owned(), path.to_string_lossy().into_owned()])
        .chain(config.flags(language).iter().flat_map(|flag| match flag {
            NativeCompileFlag::Argument(argument) => vec![argument.clone()],
            NativeCompileFlag::Include { kind, path } => {
                vec![
                    kind.argument().to_owned(),
                    path.to_string_lossy().into_owned(),
                ]
            }
        }))
        .collect()
}
