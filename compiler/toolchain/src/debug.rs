//! Host tools used only when producing an independent native debug file.

use std::path::{Path, PathBuf};

use crate::ToolchainError;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DebugFileKind {
    Dsym,
    Elf,
}

impl DebugFileKind {
    pub fn output_path(self, executable: &Path) -> PathBuf {
        let mut name = executable.as_os_str().to_os_string();
        name.push(match self {
            Self::Dsym => ".dSYM",
            Self::Elf => ".debug",
        });
        PathBuf::from(name)
    }
}

pub enum NativeDebugTools {
    Darwin {
        dsymutil: PathBuf,
        strip: PathBuf,
        codesign: PathBuf,
        inputs: Vec<PathBuf>,
    },
    Elf {
        objcopy: PathBuf,
        inputs: Vec<PathBuf>,
    },
}

impl NativeDebugTools {
    pub fn resolve(target: scoop_lir::LirTargetProfile) -> Result<Self, ToolchainError> {
        let mut inputs = Vec::new();
        match target.native_object_format() {
            scoop_lir::NativeObjectFormat::MachO64 => {
                let dsymutil = llvm_tool("dsymutil", &mut inputs)?;
                let strip = PathBuf::from("/usr/bin/strip");
                let codesign = PathBuf::from("/usr/bin/codesign");
                inputs.extend([strip.clone(), codesign.clone()]);
                Ok(Self::Darwin {
                    dsymutil,
                    strip,
                    codesign,
                    inputs,
                })
            }
            scoop_lir::NativeObjectFormat::Elf64 => Ok(Self::Elf {
                objcopy: llvm_tool("llvm-objcopy", &mut inputs)?,
                inputs,
            }),
        }
    }

    pub fn kind(&self) -> DebugFileKind {
        match self {
            Self::Darwin { .. } => DebugFileKind::Dsym,
            Self::Elf { .. } => DebugFileKind::Elf,
        }
    }

    pub fn input_paths(&self) -> &[PathBuf] {
        match self {
            Self::Darwin { inputs, .. } | Self::Elf { inputs, .. } => inputs,
        }
    }
}

/// Capture lookup configuration without locating or starting any tools.
pub fn debug_tool_configuration() -> Vec<Option<Vec<u8>>> {
    ["LLVM_SYS_221_PREFIX", "PATH"]
        .iter()
        .map(|name| std::env::var_os(name).map(|value| value.as_encoded_bytes().to_vec()))
        .collect()
}

fn llvm_tool(name: &str, inputs: &mut Vec<PathBuf>) -> Result<PathBuf, ToolchainError> {
    let mut candidates = Vec::new();
    if let Some(prefix) = std::env::var_os("LLVM_SYS_221_PREFIX") {
        candidates.push(PathBuf::from(prefix).join("bin").join(name));
    }
    candidates.push(crate::installed_executable()?.with_file_name(name));
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|root| root.join(name)));
    }
    for candidate in candidates {
        let candidate = std::path::absolute(candidate)
            .map_err(|error| ToolchainError(format!("debug tool path: {error}")))?;
        let exists = candidate.is_file();
        inputs.push(candidate.clone());
        if exists {
            return Ok(candidate);
        }
    }
    Err(ToolchainError(format!(
        "debug tool {name} not found beside Scoop or on PATH; set LLVM_SYS_221_PREFIX to the LLVM 22.1 installation"
    )))
}
