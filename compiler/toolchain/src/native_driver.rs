//! Native compiler invocation dialects and ordinary discovery output.
use crate::{ToolchainError, ValidatedCBridgeToolchainInvocation};
use scoop_protocol::HostPathCarrier;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum NativeDriverKind {
    #[default]
    Gnu,
    ClangCl,
    Msvc,
}

impl NativeDriverKind {
    pub(crate) fn microsoft(self) -> bool {
        matches!(self, Self::ClangCl | Self::Msvc)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct NativeDriverDiscovery {
    pub(crate) driver: HostPathCarrier,
    pub(crate) configuration: [u8; 32],
    #[serde(default)]
    pub(crate) kind: NativeDriverKind,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct NativeDriver {
    #[serde(with = "crate::build_record::values::path")]
    path: PathBuf,
    #[serde(with = "crate::build_record::values::digest")]
    fingerprint: scoop_wire::Digest256,
    inputs: Vec<crate::NativeInputMetadata>,
    kind: NativeDriverKind,
}

impl NativeDriver {
    pub(crate) fn from_record(
        record: crate::build_record::Record<NativeDriverDiscovery>,
    ) -> Result<Self, ToolchainError> {
        Ok(Self {
            path: record
                .result
                .driver
                .to_path_buf()
                .map_err(|e| ToolchainError(e.to_string()))?,
            fingerprint: crate::build_record::fingerprint(&(&record.result, &record.inputs))?,
            inputs: record.inputs,
            kind: record.result.kind,
        })
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn fingerprint(&self) -> scoop_wire::Digest256 {
        self.fingerprint
    }
    pub(crate) fn inputs(&self) -> &[crate::NativeInputMetadata] {
        &self.inputs
    }
    pub(crate) fn kind(&self) -> NativeDriverKind {
        self.kind
    }
    pub(crate) fn command(
        &self,
        c: &ValidatedCBridgeToolchainInvocation,
        language: scoop_manifest::NativeSourceLanguage,
    ) -> std::process::Command {
        if crate::windows_native::is_windows(c) {
            crate::windows_native::command(c, &self.path, self.kind, language, false)
        } else {
            c.driver_command_with(&self.path)
        }
    }
}
