//! The C++ driver paired with an already selected C toolchain.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use scoop_lir::{TargetProfileId, ValidatedCBridgeToolchainInvocation};
use scoop_process::CommandExt;
use scoop_wire::{Digest256, Encoder, WireEncode, domain_separated_cbor_hash, sha256};

use crate::ToolchainError;

mod discovery;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCxxToolchain {
    driver: PathBuf,
    fingerprint: Digest256,
}

impl ValidatedCxxToolchain {
    pub fn resolve(c: &ValidatedCBridgeToolchainInvocation) -> Result<Self, ToolchainError> {
        let target = c.profile().contract().target().id();
        if target == TargetProfileId::LinuxX86_64Musl {
            return Err(error("native.cxx = true is not supported for Linux musl"));
        }
        let driver = discovery::paired_driver(c)?;
        let mut inputs = vec![sha256(&std::fs::read(&driver).map_err(error)?)];
        discovery::inputs(c, &driver, &mut inputs)?;
        let macros = run(c
            .driver_command_with(&driver)
            .args(["-dM", "-E", "-x", "c++", "-include", "string", "-"])
            .stdin(Stdio::null()))?;
        inputs.push(sha256(&macros.stdout));
        let fingerprint =
            domain_separated_cbor_hash("scoop-native-cxx-toolchain-v1", &InputDigests(&inputs))
                .map_err(error)?;
        Ok(Self {
            driver,
            fingerprint,
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

struct InputDigests<'a>(&'a [Digest256]);

impl WireEncode for InputDigests<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(self.0.len() as u64)?;
        for digest in self.0 {
            digest.encode(e)?;
        }
        Ok(())
    }
}
