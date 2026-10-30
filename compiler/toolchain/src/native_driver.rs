//! Native compiler invocation dialects and ordinary discovery output.
use crate::{ToolchainError, ValidatedCBridgeToolchainInvocation};
use scoop_protocol::HostPathCarrier;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum NativeCompilerFamily {
    Gcc,
    Clang,
    Msvc,
}

impl NativeCompilerFamily {
    pub(crate) fn from_gnu_macros(macros: &str) -> Result<Self, ToolchainError> {
        if macros
            .lines()
            .any(|line| line.starts_with("#define __clang__ "))
        {
            Ok(Self::Clang)
        } else if macros
            .lines()
            .any(|line| line.starts_with("#define __GNUC__ "))
        {
            Ok(Self::Gcc)
        } else {
            Err(ToolchainError(
                "GNU-style native driver must be GCC or Clang".into(),
            ))
        }
    }
}

pub(crate) fn macro_number(macros: &str, name: &str) -> Result<u32, ToolchainError> {
    let prefix = format!("#define {name} ");
    macros
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| ToolchainError(format!("native compiler did not define numeric {name}")))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum NativeDriverKind {
    /// GNU command-line syntax, used by both GCC and Clang.
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
    pub(crate) family: NativeCompilerFamily,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct NativeDriver {
    #[serde(with = "crate::build_record::values::path")]
    path: PathBuf,
    #[serde(with = "crate::build_record::values::digest")]
    fingerprint: scoop_wire::Digest256,
    inputs: Vec<crate::NativeInputMetadata>,
    kind: NativeDriverKind,
    family: NativeCompilerFamily,
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
            family: record.result.family,
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
    pub(crate) fn family(&self) -> NativeCompilerFamily {
        self.family
    }
    pub(crate) fn command(
        &self,
        c: &ValidatedCBridgeToolchainInvocation,
        language: scoop_manifest::NativeSourceLanguage,
    ) -> std::process::Command {
        if crate::windows_native::is_windows(c) {
            crate::windows_native::command(c, &self.path, self.kind, language, false)
        } else {
            c.driver_command_for(&self.path, self.family)
        }
    }
}
