use scoop_process::CommandExt;
use std::io::Write;
use std::path::{Path, PathBuf};

use scoop_slib::ProgramLinkClosure;
use scoop_toolchain::ValidatedFinalLinkProfile;
use scoop_wire::{Digest256, Encoder, WireEncode, domain_separated_cbor_hash, sha256};

use crate::{LinkError, RuntimeObjectSet, error, program::ProgramInputs, startup::StartupObject};

mod darwin;
mod elf;
pub(crate) mod map;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedLinkPlanFingerprint(Digest256);
impl std::fmt::Display for ResolvedLinkPlanFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug)]
pub struct ProgramLinkOutput {
    pub path: PathBuf,
    pub fingerprint: ResolvedLinkPlanFingerprint,
    pub plan_dump: String,
    /// Actual native files used by this link, for destination alias checks.
    pub input_paths: Vec<PathBuf>,
}

pub fn link_program(
    closure: &ProgramLinkClosure,
    runtime: &RuntimeObjectSet,
    profile: &ValidatedFinalLinkProfile,
    library_paths: &[PathBuf],
    output: &Path,
) -> Result<ProgramLinkOutput, LinkError> {
    let inputs = ProgramInputs::new(closure, runtime, profile, library_paths)?;
    link_inputs(closure, runtime, profile, &inputs, output)
}

fn link_inputs(
    closure: &ProgramLinkClosure,
    runtime: &RuntimeObjectSet,
    profile: &ValidatedFinalLinkProfile,
    inputs: &ProgramInputs<'_>,
    output: &Path,
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
        let path = directory.path().join(format!("{}.o", input.origin));
        write_new(&path, input.bytes())?;
        paths.push(path);
    }
    let candidate = directory.path().join("program");
    let link_map = directory.path().join("program.map");
    match profile {
        ValidatedFinalLinkProfile::Darwin(_) => darwin::link(
            profile,
            inputs,
            &startup,
            directory.path(),
            &paths,
            &candidate,
            &link_map,
        )?,
        ValidatedFinalLinkProfile::Linux(linux) => elf::link(
            linux,
            inputs,
            directory.path(),
            &paths,
            &candidate,
            &link_map,
        )?,
    }
    let profile_fingerprint = profile.fingerprint().map_err(error)?;
    let fingerprint = ResolvedLinkPlanFingerprint(
        domain_separated_cbor_hash(
            "scoop-resolved-link-plan-v3",
            &Plan {
                closure,
                runtime,
                inputs,
                startup: &startup,
                profile: profile_fingerprint,
            },
        )
        .map_err(error)?,
    );
    let plan_dump = dump(closure, runtime, inputs, &startup);
    std::fs::File::open(&candidate)
        .and_then(|file| file.sync_all())
        .map_err(error)?;
    std::fs::rename(candidate, &output).map_err(|err| {
        error(format!(
            "cannot publish executable {}: {err}",
            output.display()
        ))
    })?;
    Ok(ProgramLinkOutput {
        path: output,
        fingerprint,
        plan_dump,
        input_paths: inputs
            .native
            .files
            .values()
            .map(|file| file.locator.clone())
            .chain(inputs.namespace.input_paths())
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
}
impl WireEncode for Plan<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(9)?;
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
            sha256(input.bytes()).encode(e)?;
        }
        e.field(7)?;
        e.array(2)?;
        e.text(&self.inputs.string_target)?;
        e.text(&self.inputs.symbol("scoop_td_String"))?;
        e.field(8)?;
        self.inputs.native.encode(e)?;
        e.field(9)?;
        self.inputs.namespace.encode(e)?;
        Ok(())
    }
}

fn dump(
    closure: &ProgramLinkClosure,
    runtime: &RuntimeObjectSet,
    inputs: &ProgramInputs<'_>,
    startup: &StartupObject,
) -> String {
    let mut text = String::from("program-link v3\n");
    for (artifact, symbols) in closure.artifacts() {
        text.push_str(&format!(
            "cone {} kind={:?} objects={}\n",
            artifact.manifest().cone().coordinate(),
            artifact.manifest().cone().kind(),
            symbols
                .final_objects()
                .objects()
                .iter()
                .filter(|object| inputs
                    .selected
                    .contains(artifact.identity(), object.member()))
                .count()
                + symbols.object_contents().generated_objects().len()
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
