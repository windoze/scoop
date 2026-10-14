//! The C++ driver paired with an already selected C toolchain.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::ValidatedCBridgeToolchainInvocation;
use scoop_process::CommandExt;
use scoop_wire::{Digest256, sha256};

use crate::ToolchainError;

mod discovery;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCxxToolchain {
    driver: PathBuf,
    fingerprint: Digest256,
}

impl ValidatedCxxToolchain {
    pub fn resolve(c: &ValidatedCBridgeToolchainInvocation) -> Result<Self, ToolchainError> {
        Self::from_record(Self::probe(c)?)
    }

    pub fn resolve_cached(
        target: &crate::ResolvedTargetProfile,
        cache: &Path,
    ) -> Result<Self, ToolchainError> {
        let key = crate::build_record::fingerprint(
            &crate::native_record::ResolvedRecord::from_target(target)?,
        )?;
        let record = crate::build_record::cached(
            &cache.join("cxx-tools-v1"),
            key,
            |_| Ok(true),
            || Self::probe(target.c_bridge_toolchain()),
        )?;
        Self::from_record(record)
    }

    fn from_record(record: crate::build_record::Record<Discovery>) -> Result<Self, ToolchainError> {
        Ok(Self {
            driver: record.result.driver.to_path_buf().map_err(error)?,
            fingerprint: crate::build_record::fingerprint(&(
                record.result.configuration,
                record.inputs,
            ))?,
        })
    }

    fn probe(
        c: &ValidatedCBridgeToolchainInvocation,
    ) -> Result<crate::build_record::Record<Discovery>, ToolchainError> {
        if c.profile().contract().target().id() == scoop_target_data::LINUX_X86_64_MUSL_ID {
            return Err(error("native.cxx = true is not supported for Linux musl"));
        }
        let driver = discovery::paired_driver(c)?;
        let mut inputs = std::collections::BTreeSet::new();
        discovery::inputs(c, &driver, &mut inputs)?;
        let headers = run(c
            .driver_command_with(&driver)
            .args([
                "-v",
                "-M",
                "-MT",
                "cxx-probe",
                "-x",
                "c++",
                "-include",
                "string",
                "-",
            ])
            .stdin(Stdio::null()))?;
        let headers_text = std::str::from_utf8(&headers.stdout).map_err(error)?;
        let mut header_paths = scoop_process::parse_make_dependencies(headers_text, "cxx-probe")
            .ok_or_else(|| error("C++ compiler returned an invalid header dependency list"))?
            .into_iter()
            .filter(|p| p != Path::new("-") && p != Path::new("<stdin>"))
            .collect();
        crate::native_record::header_candidates(&headers.stderr, &mut header_paths);
        inputs.extend(header_paths);
        inputs.insert(driver.clone());
        let macros = run(c
            .driver_command_with(&driver)
            .args(["-dM", "-E", "-x", "c++", "-include", "string", "-"])
            .stdin(Stdio::null()))?;
        Ok(crate::build_record::Record {
            inputs: inputs
                .iter()
                .map(|p| crate::build_record::Input::read(p))
                .collect::<Result<_, _>>()?,
            result: Discovery {
                driver: scoop_protocol::HostPathCarrier::from_path(&driver).map_err(error)?,
                configuration: *sha256(&macros.stdout).as_array(),
            },
        })
    }

    pub fn driver(&self) -> &Path {
        &self.driver
    }

    pub fn fingerprint(&self) -> Digest256 {
        self.fingerprint
    }

    pub fn command(&self, c: &ValidatedCBridgeToolchainInvocation) -> Command {
        c.driver_command_with(&self.driver)
    }
}

fn run(command: &mut Command) -> Result<std::process::Output, ToolchainError> {
    let output = command.scoop_output().map_err(error)?;
    if !output.status.success() {
        return Err(error(String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(output)
}

fn text(command: &mut Command) -> Result<String, ToolchainError> {
    String::from_utf8(run(command)?.stdout)
        .map(|text| text.trim().to_owned())
        .map_err(error)
}

fn error(value: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("native C++ toolchain: {value}"))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Discovery {
    driver: scoop_protocol::HostPathCarrier,
    configuration: [u8; 32],
}
