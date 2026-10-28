//! Reuse a completed executable before reading Link artifacts or preparing tools.
use super::*;
use fs4::FileExt;
use scoop_linker::ResolvedLinkPlanFingerprint;
use scoop_toolchain::NativeInputMetadata;
use std::collections::BTreeSet;

mod key;
mod record;

pub(super) struct Request<'a> {
    pub graph: &'a crate::BuildGraphOutcome,
    pub runtime: &'a RuntimeObjectSet,
    pub target: &'a ResolvedTargetProfile,
    pub cache: &'a Path,
    pub profile: crate::BuildProfile,
    pub options: &'a scoop_toolchain::FinalLinkOptions,
    pub libraries: &'a [PathBuf],
    pub explicit_runtime: bool,
    pub output: &'a Path,
}

impl Request<'_> {
    fn keep_symbols(&self) -> bool {
        self.options.keep_symbols || self.profile == crate::BuildProfile::Debug
    }

    fn emit_symbol_map(&self) -> bool {
        self.options.emit_symbol_map || self.profile == crate::BuildProfile::Debug
    }

    fn emit_debug_file(&self) -> bool {
        self.options.emit_debug_file
    }
}

pub(super) struct Executable {
    pub path: PathBuf,
    pub symbol_map: Option<PathBuf>,
    pub debug_file: Option<scoop_linker::DebugFile>,
    pub fingerprint: ResolvedLinkPlanFingerprint,
    pub result_key: [u8; 32],
}

pub(super) fn get(request: Request<'_>) -> BuildResult<Executable> {
    let key = key::of(&request)?;
    let namespace = request.cache.join("executables-v4");
    let entry = namespace.join(format!("{key}.json"));
    if let Some(executable) = record::load(&entry)? {
        return Ok(executable);
    }
    std::fs::create_dir_all(&namespace).map_err(failure)?;
    let lock =
        crate::cache::open_build_lock(&namespace.join(format!("{key}.lock"))).map_err(failure)?;
    FileExt::lock(&lock).map_err(failure)?;
    if let Some(executable) = record::load(&entry)? {
        return Ok(executable);
    }
    let directory = tempfile::Builder::new()
        .prefix("program-")
        .tempdir_in(&namespace)
        .map_err(failure)?;
    let closure = read_built_program(request.graph.closure(), request.target).map_err(failure)?;
    let profile = scoop_linker::resolve_program_link_profile(
        &closure,
        request.target,
        request.options,
        Some(request.cache),
    )
    .map_err(|error| {
        BuildFailure::tool("SCOOP_LINK_TOOLCHAIN", BuildFailurePhase::FinalLink, error)
    })?;
    let linked = scoop_linker::link_program(
        &closure,
        request.runtime,
        &profile,
        scoop_linker::ProgramLinkOptions {
            keep_symbols: request.keep_symbols(),
            emit_symbol_map: request.emit_symbol_map(),
            emit_debug_file: request.emit_debug_file(),
        },
        request.libraries,
        &directory.path().join(if request.emit_debug_file() {
            request
                .output
                .file_name()
                .ok_or_else(|| failure("executable has no basename"))?
        } else {
            std::ffi::OsStr::new("program")
        }),
        request.cache,
    )
    .map_err(failure)?;
    let mut paths: BTreeSet<_> = linked.input_paths.into_iter().collect();
    paths.extend(profile.input_paths().map_err(failure)?);
    if request.explicit_runtime {
        paths.extend(request.runtime.input_paths().iter().cloned());
    }
    let inputs = paths
        .iter()
        .map(|path| NativeInputMetadata::read(path).map_err(failure))
        .collect::<BuildResult<Vec<_>>>()?;
    let result_key =
        *scoop_wire::sha256(&serde_json::to_vec(&(key.as_array(), &inputs)).map_err(failure)?)
            .as_array();
    let executable = Executable {
        path: linked.path,
        symbol_map: linked.symbol_map,
        debug_file: linked.debug_file,
        fingerprint: linked.fingerprint,
        result_key,
    };
    let _ = directory.keep();
    record::save(&entry, &executable, inputs)?;
    Ok(executable)
}

fn failure(error: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool("SCOOP_LINK_FAILED", BuildFailurePhase::FinalLink, error)
}
