use scoop_process::CommandExt;
use std::io::Write;
use std::path::{Path, PathBuf};

use scoop_slib::ProgramLinkClosure;
use scoop_toolchain::ValidatedFinalLinkProfile;
use scoop_wire::{Digest256, Encoder, WireEncode, domain_separated_cbor_hash, sha256};

use crate::{LinkError, RuntimeObjectSet, error, program::ProgramInputs, startup::StartupObject};

mod darwin;
mod elf;
mod pe;
mod response;

#[cfg(test)]
mod tests;
#[cfg(all(test, windows, target_arch = "x86_64"))]
mod windows_tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedLinkPlanFingerprint(Digest256);
impl ResolvedLinkPlanFingerprint {
    pub const fn as_array(&self) -> &[u8; 32] {
        self.0.as_array()
    }
    pub const fn from_array(bytes: [u8; 32]) -> Self {
        Self(Digest256::from_array(bytes))
    }
}
impl std::fmt::Display for ResolvedLinkPlanFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProgramLinkOptions<'a> {
    pub keep_symbols: bool,
    pub emit_symbol_map: bool,
    /// Published PDB basename when the executable is first linked in a cache.
    pub pdb_name: Option<&'a std::ffi::OsStr>,
}

#[derive(Debug)]
pub struct ProgramLinkOutput {
    pub path: PathBuf,
    /// Native linker address map for this exact executable.
    pub symbol_map: Option<PathBuf>,
    /// CodeView/PDB sidecar for a Windows executable with retained symbols.
    pub pdb: Option<PathBuf>,
    pub fingerprint: ResolvedLinkPlanFingerprint,
    pub plan_dump: String,
    /// Actual native files and lookup candidates used by this link.
    pub input_paths: Vec<PathBuf>,
}

pub fn link_program(
    closure: &ProgramLinkClosure,
    runtime: &RuntimeObjectSet,
    profile: &ValidatedFinalLinkProfile,
    options: ProgramLinkOptions<'_>,
    library_paths: &[PathBuf],
    output: &Path,
    cache: &Path,
) -> Result<ProgramLinkOutput, LinkError> {
    let inputs = ProgramInputs::new(closure, runtime, profile, library_paths)?;
    link_inputs(closure, runtime, profile, options, &inputs, output, cache)
}

fn link_inputs(
    closure: &ProgramLinkClosure,
    runtime: &RuntimeObjectSet,
    profile: &ValidatedFinalLinkProfile,
    options: ProgramLinkOptions<'_>,
    inputs: &ProgramInputs<'_>,
    output: &Path,
    cache: &Path,
) -> Result<ProgramLinkOutput, LinkError> {
    let output = std::path::absolute(output).map_err(error)?;
    let parent = output
        .parent()
        .ok_or_else(|| error("executable output has no parent directory"))?;
    std::fs::create_dir_all(parent).map_err(error)?;
    let directory = tempfile::Builder::new()
        .prefix(".scoop-link-")
        .tempdir_in(parent)
        .map_err(error)?;
    let startup = StartupObject::build(inputs, profile, directory.path())?;
    let mut paths = Vec::new();
    let first = directory.path().join("startup.o");
    write_new(&first, &startup.bytes)?;
    paths.push(first);
    for input in &inputs.objects {
        paths.push(input.path(cache)?);
    }
    let candidate = directory.path().join(
        if matches!(profile, ValidatedFinalLinkProfile::Windows(_)) {
            "program.exe"
        } else {
            "program"
        },
    );
    let candidate_map = options
        .emit_symbol_map
        .then(|| directory.path().join("program.map"));
    let candidate_pdb = (options.keep_symbols
        && matches!(profile, ValidatedFinalLinkProfile::Windows(_)))
    .then(|| {
        let default = pdb_path(&output);
        directory.path().join(
            options
                .pdb_name
                .unwrap_or_else(|| default.file_name().expect("PDB filename")),
        )
    });
    let mut link_input_paths = Vec::new();
    match profile {
        ValidatedFinalLinkProfile::Darwin(_) => darwin::link(
            profile,
            inputs,
            directory.path(),
            &paths,
            &candidate,
            options.keep_symbols,
            candidate_map.as_deref(),
        )?,
        ValidatedFinalLinkProfile::Linux(linux) => elf::link(
            linux,
            inputs,
            directory.path(),
            &paths,
            &candidate,
            options.keep_symbols,
            candidate_map.as_deref(),
        )?,
        ValidatedFinalLinkProfile::Windows(windows) => {
            link_input_paths = pe::link(
                windows,
                inputs,
                directory.path(),
                &paths,
                &candidate,
                candidate_pdb.as_deref(),
                candidate_map.as_deref(),
            )?;
        }
    }
    let profile_fingerprint = profile.fingerprint().map_err(error)?;
    let fingerprint = ResolvedLinkPlanFingerprint(
        domain_separated_cbor_hash(
            "scoop-resolved-link-plan-v6",
            &Plan {
                closure,
                runtime,
                inputs,
                startup: &startup,
                profile: profile_fingerprint,
                keep_symbols: options.keep_symbols,
                pdb_name: candidate_pdb.as_deref().and_then(Path::file_name),
            },
        )
        .map_err(error)?,
    );
    let plan_dump = dump(closure, runtime, inputs, &startup, options.keep_symbols);
    let symbol_map = candidate_map
        .map(|candidate| {
            let symbol_map = symbol_map_path(&output);
            std::fs::rename(candidate, &symbol_map).map_err(error)?;
            Ok::<_, LinkError>(symbol_map)
        })
        .transpose()?;
    let pdb = candidate_pdb
        .map(|candidate| {
            let path = output
                .parent()
                .expect("absolute output parent")
                .join(candidate.file_name().expect("PDB filename"));
            std::fs::rename(candidate, &path).map_err(error)?;
            Ok::<_, LinkError>(path)
        })
        .transpose()?;
    std::fs::rename(candidate, &output).map_err(|err| {
        error(format!(
            "cannot publish executable {}: {err}",
            output.display()
        ))
    })?;
    Ok(ProgramLinkOutput {
        path: output,
        symbol_map,
        pdb,
        fingerprint,
        plan_dump,
        input_paths: inputs
            .native
            .input_paths()
            .into_iter()
            .chain(inputs.namespace.input_paths())
            .chain(link_input_paths)
            .collect(),
    })
}

/// Sidecar naming is shared by final linking and cached output publication.
pub fn symbol_map_path(output: &Path) -> PathBuf {
    let mut path = output.as_os_str().to_os_string();
    path.push(".map");
    PathBuf::from(path)
}

pub fn pdb_path(output: &Path) -> PathBuf {
    let mut path = output.as_os_str().to_os_string();
    path.push(".pdb");
    PathBuf::from(path)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), LinkError> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(error)?;
    file.write_all(bytes).map_err(error)
}

struct Plan<'a> {
    closure: &'a ProgramLinkClosure,
    runtime: &'a RuntimeObjectSet,
    inputs: &'a ProgramInputs<'a>,
    startup: &'a StartupObject,
    profile: Digest256,
    keep_symbols: bool,
    pdb_name: Option<&'a std::ffi::OsStr>,
}
impl WireEncode for Plan<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(12)?;
        e.field(1)?;
        self.closure.root().encode(e)?;
        e.field(2)?;
        e.array(self.closure.artifacts().len() as u64)?;
        for (artifact, symbols) in self.closure.artifacts() {
            e.array(2)?;
            artifact.identity().encode(e)?;
            symbols.code_fingerprint().encode(e)?;
        }
        e.field(3)?;
        self.runtime.fingerprint().digest().encode(e)?;
        e.field(4)?;
        self.profile.encode(e)?;
        e.field(5)?;
        e.array(2)?;
        e.text(&self.startup.source)?;
        sha256(&self.startup.bytes).encode(e)?;
        e.field(6)?;
        e.array(self.inputs.objects.len() as u64)?;
        for input in &self.inputs.objects {
            e.array(2)?;
            e.text(&input.origin.to_string())?;
            input.digest().encode(e)?;
        }
        e.field(7)?;
        e.array(2)?;
        e.text(&self.inputs.string_target)?;
        e.text(&self.inputs.symbol("scoop_td_String"))?;
        e.field(8)?;
        self.inputs.native.encode(e)?;
        e.field(9)?;
        self.inputs.namespace.encode(e)?;
        e.field(10)?;
        e.unsigned(u64::from(self.keep_symbols))?;
        e.field(11)?;
        match self.pdb_name {
            Some(name) => {
                e.array(1)?;
                e.bytes(name.as_encoded_bytes())?;
            }
            None => e.array(0)?,
        }
        e.field(12)?;
        self.inputs.windows_subsystem.encode(e)?;
        Ok(())
    }
}

fn dump(
    closure: &ProgramLinkClosure,
    runtime: &RuntimeObjectSet,
    inputs: &ProgramInputs<'_>,
    startup: &StartupObject,
    keep_symbols: bool,
) -> String {
    let mut text = format!(
        "program-link v3\nsymbols={}\n",
        if keep_symbols { "keep" } else { "strip" }
    );
    for (artifact, symbols) in closure.artifacts() {
        text.push_str(&format!(
            "cone {} kind={:?} objects={}\n",
            artifact.manifest().cone().coordinate(),
            artifact.manifest().cone().kind(),
            symbols
                .members()
                .iter()
                .filter(|object| inputs
                    .selected
                    .contains(artifact.identity(), object.member()))
                .count()
        ));
    }
    text.push_str(&format!(
        "runtime objects={}\nimages={} root={}\nalias {} -> {}\n",
        runtime.objects().len(),
        inputs.images.len(),
        inputs.root,
        inputs.symbol("scoop_td_String"),
        inputs.string_target
    ));
    text.push_str(match &inputs.namespace {
        crate::namespace::NativeNamespace::Darwin(_) => "link inputs: startup, cone/member order, runtime/object order, libSystem\n",
        crate::namespace::NativeNamespace::Elf(_) => "link inputs: startup, cone/member order, runtime/object order, native CRT/libc/LLVM unwind\n",
        crate::namespace::NativeNamespace::Coff(_) => "link inputs: Unicode startup, cone/member order, runtime/object order, COFF/import libraries, selected Windows CRT/LLVM unwind\n",
    });
    text.push_str(&inputs.native.dump());
    text.push_str(&inputs.namespace.dump());
    text.push_str("startup object references:\n");
    for symbol in &startup.references {
        text.push_str(&format!("  {symbol}\n"));
    }
    text.push_str(&startup.source);
    text
}
