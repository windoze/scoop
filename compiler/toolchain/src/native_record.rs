//! Ordinary C-tool discovery results, shared by parent and compiler requests.
use std::fs::{File, OpenOptions};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use scoop_protocol::{HostPathCarrier, TargetSelectionRequestV1};
use scoop_wire::{Digest256, sha256};
use serde::{Deserialize, Serialize};

use crate::{
    CToolchainOptions, ResolvedTargetProfile, ToolchainError, ValidatedCBridgeToolchainInvocation,
};

pub(crate) mod inputs;
mod invocation;
use inputs::Input;
pub(crate) use inputs::{header_candidates, program_candidates};
use invocation::InvocationRecord;

#[derive(Serialize, Deserialize)]
pub(crate) struct ResolvedRecord {
    invocation: InvocationRecord,
    input_fingerprint: [u8; 32],
}

impl ResolvedRecord {
    pub(crate) fn from_target(target: &ResolvedTargetProfile) -> Result<Self, ToolchainError> {
        Ok(Self {
            invocation: InvocationRecord::from_invocation(target.c_bridge_toolchain())?,
            input_fingerprint: *target.native_tool_inputs().as_array(),
        })
    }

    pub(crate) fn into_target(self, triple: &str) -> Result<ResolvedTargetProfile, ToolchainError> {
        let target = crate::registry::select_target(triple)?;
        ResolvedTargetProfile::from_c_toolchain(
            target,
            self.invocation.into_invocation(target.target())?,
            Digest256::from_array(self.input_fingerprint),
        )
    }
}

#[derive(Serialize, Deserialize)]
struct Record {
    inputs: Vec<Input>,
    result: ResolvedRecord,
}

pub(crate) fn probe(
    triple: &str,
    options: &CToolchainOptions,
) -> Result<ResolvedTargetProfile, ToolchainError> {
    probe_record(triple, options)?.result.into_target(triple)
}

fn probe_record(triple: &str, options: &CToolchainOptions) -> Result<Record, ToolchainError> {
    let target = crate::registry::select_target(triple)?.target();
    let (invocation, paths) = match target.id() {
        scoop_target_data::DARWIN_AARCH64_ID => {
            if options.compiler.is_some() || options.native_sysroot.is_some() {
                return Err(ToolchainError("--cc and --native-sysroot apply to Linux targets; Darwin uses the selected Xcode toolchain".into()));
            }
            crate::c_bridge::probe_system_c_bridge_toolchain()?
        }
        _ => crate::linux_c::probe_linux_c_toolchain(
            target,
            options.compiler.as_deref(),
            options.native_sysroot.as_deref(),
        )?,
    };
    let inputs = paths
        .iter()
        .map(|path| Input::read(path))
        .collect::<Result<Vec<_>, _>>()?;
    let input_fingerprint = *sha256(&serde_json::to_vec(&inputs).map_err(error)?).as_array();
    Ok(Record {
        inputs,
        result: ResolvedRecord {
            invocation: InvocationRecord::from_invocation(&invocation)?,
            input_fingerprint,
        },
    })
}

pub(crate) fn resolve_cached(
    request: &TargetSelectionRequestV1,
    cache: &Path,
) -> Result<ResolvedTargetProfile, ToolchainError> {
    let triple = request.canonical_triple();
    crate::registry::select_target(triple)?;
    let options = CToolchainOptions::from_request(request)?;
    let environment = if triple.contains("linux") {
        vec![
            std::env::var_os("PATH"),
            triple
                .ends_with("musl")
                .then(|| std::env::var_os("REALGCC"))
                .flatten(),
        ]
    } else {
        vec![Some(crate::c_bridge::deployment_version()?.into())]
    };
    let environment: Vec<_> = environment
        .iter()
        .map(|v| v.as_ref().map(|v| v.as_encoded_bytes()))
        .collect();
    let key = sha256(
        &serde_json::to_vec(&(scoop_wire::encode(request).map_err(error)?, environment))
            .map_err(error)?,
    );
    let directory = cache.join("native-tools-v1");
    let record_path = directory.join(format!("{key}.json"));
    if let Some(result) = lookup(&record_path)? {
        return result.into_target(triple);
    }
    std::fs::create_dir_all(&directory).map_err(error)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(format!("{key}.lock")))
        .map_err(error)?;
    fs4::FileExt::lock(&lock).map_err(error)?;
    if let Some(result) = lookup(&record_path)? {
        return result.into_target(triple);
    }
    let record = probe_record(triple, &options)?;
    let mut output = tempfile::NamedTempFile::new_in(&directory).map_err(error)?;
    serde_json::to_writer(&mut output, &record).map_err(error)?;
    output.persist(record_path).map_err(|e| error(e.error))?;
    record.result.into_target(triple)
}

fn lookup(path: &Path) -> Result<Option<ResolvedRecord>, ToolchainError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(error(e)),
    };
    let record: Record = serde_json::from_reader(BufReader::new(file)).map_err(error)?;
    for input in &record.inputs {
        if !input.is_current()? {
            return Ok(None);
        }
    }
    Ok(Some(record.result))
}

fn path(path: &Path) -> Result<HostPathCarrier, ToolchainError> {
    HostPathCarrier::from_path(path).map_err(error)
}

fn error(value: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("native toolchain record: {value}"))
}

#[cfg(test)]
mod tests;
