use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::native_driver::NativeDriverKind;
use scoop_lir::{OptimizationMode, TargetProfileId};
use scoop_manifest::{LoadedConeManifest, NativeCompileFlag, NativeConfig, NativeSourceLanguage};

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
    let target_facts = crate::manifest::source_target(target)?;
    let sources = sources
        .into_iter()
        .map(|source| {
            config
                .source_language(source.path(), target_facts)
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
    let mut result = PreparedNativeInputs {
        libraries: libraries.clone(),
        cxx,
        ..PreparedNativeInputs::default()
    };
    let mut search = vec![roots.cone.clone(), roots.public.clone()];
    search.extend(roots.system.clone());
    search.extend(
        config
            .include_paths(target)
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
        let mut command = command::base(compiler, *language, optimization, false)?;
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
                target_facts.env.to_ascii_uppercase().replace('-', "_")
            ),
        ]);
        let kind = compiler.kind(*language);
        command::configure_preprocessor(
            &mut command,
            config,
            target,
            *language,
            &roots.cone,
            kind,
        )?;
        command
            .current_dir(dunce::simplified(&roots.cone))
            .arg(if kind.microsoft() { "/I" } else { "-I" })
            .arg(dunce::simplified(&roots.public));
        for (root, logical) in roots.mappings() {
            command::remap(&mut command, kind, root, &logical);
        }
        if kind.microsoft() {
            command.arg("/E").arg(match language {
                NativeSourceLanguage::C => "/TC",
                NativeSourceLanguage::Cxx => "/TP",
                NativeSourceLanguage::ObjectiveC => {
                    return Err(error("Objective-C sources require a Darwin target"));
                }
            });
            if kind == NativeDriverKind::ClangCl {
                command.arg("/clang:-dI");
            }
        } else {
            command.args(["-E", "-dI", "-x", language.input_name()]);
        }
        crate::windows_native::dependencies(&mut command, kind, &depfile, "native-unit");
        crate::windows_native::input(&mut command, kind, &input);
        let output = command::run(&mut command, source.path().as_str())?;
        let dependencies = crate::windows_native::read_dependencies(kind, &depfile, "native-unit")?;
        if kind == NativeDriverKind::Msvc {
            observe_msvc_search(&search, &dependencies, &mut result.observed_paths)?;
        }
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
        result.flags = flags(config, target, NativeSourceLanguage::C);
    }
    if languages.contains(&NativeSourceLanguage::Cxx) {
        result.cxx_flags = flags(config, target, NativeSourceLanguage::Cxx);
    }
    if languages.contains(&NativeSourceLanguage::ObjectiveC) {
        result.objc_flags = flags(config, target, NativeSourceLanguage::ObjectiveC);
    }
    Ok(result)
}

fn observe_msvc_search(
    search: &[std::path::PathBuf],
    dependencies: &[std::path::PathBuf],
    observed: &mut BTreeSet<std::path::PathBuf>,
) -> Result<(), ToolchainError> {
    observed.extend(search.iter().cloned());
    let roots: Vec<_> = search
        .iter()
        .filter(|p| p.is_dir())
        .map(|p| p.canonicalize().map_err(error))
        .collect::<Result<_, _>>()?;
    for dependency in dependencies {
        let path = dependency.canonicalize().map_err(error)?;
        if let Some(parent) = path.parent() {
            observed.insert(parent.to_owned());
        }
        for root in &roots {
            if let Ok(relative) = path.strip_prefix(root) {
                for candidate in &roots {
                    observed.insert(candidate.join(relative));
                }
            }
        }
    }
    Ok(())
}

fn flags(
    config: &NativeConfig,
    target: TargetProfileId,
    language: NativeSourceLanguage,
) -> Vec<String> {
    config
        .include_paths(target)
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
