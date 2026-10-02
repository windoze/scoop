use std::io::Write;
use std::path::{Path, PathBuf};

use scoop_slib::ProgramLinkClosure;
use scoop_toolchain::ValidatedFinalLinkProfile;
use scoop_wire::{Digest256, Encoder, WireEncode, domain_separated_cbor_hash, sha256};

use crate::{LinkError, RuntimeObjectSet, error, program::ProgramInputs, startup::StartupObject};

mod map;

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
}

pub fn link_program(
    closure: &ProgramLinkClosure,
    runtime: &RuntimeObjectSet,
    profile: &ValidatedFinalLinkProfile,
    library_paths: &[PathBuf],
    output: &Path,
) -> Result<ProgramLinkOutput, LinkError> {
    let inputs = ProgramInputs::new(closure, runtime, profile, library_paths)?;
    let output = std::path::absolute(output).map_err(error)?;
    let parent = output
        .parent()
        .ok_or_else(|| error("executable output has no parent directory"))?;
    std::fs::create_dir_all(parent).map_err(error)?;
    let directory = tempfile::Builder::new()
        .prefix(".scoop-link-")
        .tempdir_in(parent)
        .map_err(error)?;
    let startup = StartupObject::build(&inputs, profile, directory.path())?;
    let mut paths = Vec::new();
    let first = directory.path().join("startup.o");
    write_new(&first, &startup.bytes)?;
    paths.push(first);
    for input in &inputs.objects {
        let path = directory.path().join(format!("{}.o", input.origin));
        write_new(&path, input.bytes())?;
        paths.push(path);
    }
    let sdk = directory.path().join("sdk");
    std::fs::create_dir(&sdk).map_err(error)?;
    let mut stubs = std::collections::BTreeMap::new();
    for (id, bytes) in &inputs.providers.stubs {
        let path = directory.path().join(format!("dynamic-{id}.tbd"));
        write_new(&path, bytes)?;
        stubs.insert(path, inputs.providers.providers[id].install_name.clone());
    }
    let candidate = directory.path().join("program");
    let link_map = directory.path().join("program.map");
    let mut command = profile.command(&sdk, &candidate, &link_map);
    command
        .args(&paths)
        .arg("-alias")
        .arg(&inputs.string_target)
        .arg("_scoop_td_String");
    for path in &inputs.providers.rpaths {
        command.arg("-rpath").arg(path);
    }
    command.args(stubs.keys());
    let result = command
        .output()
        .map_err(|err| error(format!("cannot start system linker: {err}")))?;
    if !result.status.success() {
        return Err(error(format!(
            "system linker failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    map::check(
        &std::fs::read_to_string(link_map).map_err(error)?,
        &paths,
        &stubs,
    )?;
    map::trace(
        std::str::from_utf8(&result.stdout).map_err(error)?,
        &paths,
        &stubs,
    )?;
    let bytes = std::fs::read(&candidate).map_err(error)?;
    crate::final_image::verify(&bytes, &inputs, &startup, profile)
        .map_err(|err| error(format!("final Mach-O validation: {err}")))?;
    let profile_fingerprint = profile.fingerprint().map_err(error)?;
    let fingerprint = ResolvedLinkPlanFingerprint(
        domain_separated_cbor_hash(
            "scoop-resolved-link-plan-v2",
            &Plan {
                closure,
                runtime,
                inputs: &inputs,
                startup: &startup,
                profile: profile_fingerprint,
            },
        )
        .map_err(error)?,
    );
    let plan_dump = dump(closure, runtime, &inputs, &startup);
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
        e.text("_scoop_td_String")?;
        e.field(8)?;
        self.inputs.native.encode(e)?;
        e.field(9)?;
        self.inputs.providers.encode(&self.inputs.dynamic, e)?;
        Ok(())
    }
}

fn dump(
    closure: &ProgramLinkClosure,
    runtime: &RuntimeObjectSet,
    inputs: &ProgramInputs<'_>,
    startup: &StartupObject,
) -> String {
    let mut text = String::from("program-link v2\n");
    for (artifact, symbols) in closure.artifacts() {
        text.push_str(&format!(
            "cone {} kind={:?} objects={}\n",
            artifact.manifest().cone().coordinate(),
            artifact.manifest().cone().kind(),
            symbols.final_objects().objects().len()
                + symbols.object_contents().generated_objects().len()
        ));
    }
    text.push_str(&format!(
        "runtime objects={}\nimages={} root={}\nalias _scoop_td_String -> {}\n",
        runtime.objects().len(),
        inputs.images.len(),
        inputs.root,
        inputs.string_target
    ));
    text.push_str("link inputs: startup, cone/member order, runtime/object order, libSystem\n");
    text.push_str(&inputs.native.dump());
    text.push_str(&inputs.providers.dump(&inputs.dynamic));
    text.push_str("startup object references:\n");
    for symbol in &startup.references {
        text.push_str(&format!("  {symbol}\n"));
    }
    text.push_str(&startup.source);
    text
}
