use scoop_process::CommandExt;
use std::io::Write;
use std::path::{Path, PathBuf};

use scoop_slib::ProgramLinkClosure;
use scoop_toolchain::ValidatedFinalLinkProfile;
use scoop_wire::{Digest256, Encoder, WireEncode, domain_separated_cbor_hash, sha256};

use crate::{LinkError, RuntimeObjectSet, error, program::ProgramInputs, startup::StartupObject};

mod darwin;
mod debug;
pub use debug::{DebugFile, replace_debug_bundle};
mod elf;
mod response;

#[cfg(test)]
mod tests;

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
pub struct ProgramLinkOptions {
    pub keep_symbols: bool,
    pub emit_debug_file: bool,
}

#[derive(Debug)]
pub struct ProgramLinkOutput {
    pub path: PathBuf,
    pub debug_file: Option<DebugFile>,
    pub fingerprint: ResolvedLinkPlanFingerprint,
    pub plan_dump: String,
    /// Actual native files and lookup candidates used by this link.
    pub input_paths: Vec<PathBuf>,
}

pub fn link_program(
    closure: &ProgramLinkClosure,
    runtime: &RuntimeObjectSet,
    profile: &ValidatedFinalLinkProfile,
    options: ProgramLinkOptions,
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
    options: ProgramLinkOptions,
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
    let basename = output
        .file_name()
        .ok_or_else(|| error("executable has no basename"))?;
    let candidate = directory.path().join(basename);
    let debug_tools = options
        .emit_debug_file
        .then(|| scoop_toolchain::NativeDebugTools::resolve(profile.target()).map_err(error))
        .transpose()?;
    match profile {
        ValidatedFinalLinkProfile::Darwin(_) => darwin::link(
            profile,
            inputs,
            directory.path(),
            &paths,
            &candidate,
            options.keep_symbols || options.emit_debug_file,
        )?,
        ValidatedFinalLinkProfile::Linux(linux) => elf::link(
            linux,
            inputs,
            directory.path(),
            &paths,
            &candidate,
            options.keep_symbols || options.emit_debug_file,
        )?,
    }
    let debug_file = debug_tools
        .as_ref()
        .map(|tools| debug::generate(tools, &candidate, options.keep_symbols))
        .transpose()?;
    let profile_fingerprint = profile.fingerprint().map_err(error)?;
    let fingerprint = ResolvedLinkPlanFingerprint(
        domain_separated_cbor_hash(
            "scoop-resolved-link-plan-v5",
            &Plan {
                closure,
                runtime,
                inputs,
                startup: &startup,
                profile: profile_fingerprint,
                keep_symbols: options.keep_symbols,
                debug_basename: options.emit_debug_file.then_some(basename),
            },
        )
        .map_err(error)?,
    );
    let plan_dump = dump(closure, runtime, inputs, &startup, options.keep_symbols);
    let debug_file = debug_file
        .map(|file| {
            let path = file.kind.output_path(&output);
            match file.kind {
                scoop_toolchain::DebugFileKind::Dsym => replace_debug_bundle(&file.path, &path),
                scoop_toolchain::DebugFileKind::Elf => std::fs::rename(&file.path, &path),
            }
            .map_err(error)?;
            Ok::<_, LinkError>(DebugFile {
                kind: file.kind,
                path,
            })
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
        debug_file,
        fingerprint,
        plan_dump,
        input_paths: inputs
            .native
            .input_paths()
            .into_iter()
            .chain(inputs.namespace.input_paths())
            .chain(
                debug_tools
                    .iter()
                    .flat_map(|tools| tools.input_paths().iter().cloned()),
            )
            .collect(),
    })
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
    debug_basename: Option<&'a std::ffi::OsStr>,
}
impl WireEncode for Plan<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(11)?;
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
        e.array(u64::from(self.debug_basename.is_some()))?;
        if let Some(name) = self.debug_basename {
            e.bytes(name.as_encoded_bytes())?;
        }
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
